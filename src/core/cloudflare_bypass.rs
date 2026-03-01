use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::path::PathBuf;
use std::fs;
use anyhow::{anyhow, Result};
use futures::StreamExt;

#[derive(Clone, Debug)]
pub enum BypassResult {
    Success {
        #[allow(dead_code)]
        cf_clearance: String,
        content: String,
    },
    Cancelled,
    Error(String),
}

#[derive(Clone, Debug)]
pub enum BrowserStatus {
    Launching,
    WaitingForVerification,
    Verified,
    DownloadingContent,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct CloudflareCookieCache {
    pub cf_clearance: String,
    pub timestamp: u64,
    pub domain: String,
}

impl CloudflareCookieCache {
    /// valid for 12 hours
    pub fn is_valid(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now - self.timestamp < 43200 // 12 hours
    }
}

fn get_cache_dir() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    exe_dir.join("cache")
}

fn get_cookie_cache_path() -> PathBuf {
    get_cache_dir().join("cloudflare_cookies.json")
}

pub fn load_cached_cookie(domain: &str) -> Option<CloudflareCookieCache> {
    let path = get_cookie_cache_path();
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(cache) = serde_json::from_str::<CloudflareCookieCache>(&content) {
                if cache.domain == domain && cache.is_valid() {
                    return Some(cache);
                }
            }
        }
    }
    None
}

pub fn save_cookie_cache(cf_clearance: &str, domain: &str) {
    let cache = CloudflareCookieCache {
        cf_clearance: cf_clearance.to_string(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        domain: domain.to_string(),
    };
    
    let cache_dir = get_cache_dir();
    let _ = fs::create_dir_all(&cache_dir);
    
    if let Ok(json) = serde_json::to_string_pretty(&cache) {
        let _ = fs::write(get_cookie_cache_path(), json);
    }
}

pub struct CloudflareBypassSession {
    result_receiver: Receiver<BypassResult>,
    status_receiver: Receiver<BrowserStatus>,
    #[allow(dead_code)]
    cancel_flag: Arc<Mutex<bool>>,
}

impl CloudflareBypassSession {
    pub fn start(url: &str) -> Result<Self, String> {
        let (result_sender, result_receiver) = channel();
        let (status_sender, status_receiver) = channel();
        let cancel_flag = Arc::new(Mutex::new(false));
        let cancel_flag_clone = cancel_flag.clone();
        let url_owned = url.to_string();
        
        thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create tokio runtime");
            
            rt.block_on(async {
                run_cloudflare_bypass_session(
                    &url_owned,
                    result_sender,
                    status_sender,
                    cancel_flag_clone,
                ).await;
            });
        });
        
        Ok(Self {
            result_receiver,
            status_receiver,
            cancel_flag,
        })
    }
    
    /// non-blocking
    pub fn try_get_result(&self) -> Option<BypassResult> {
        match self.result_receiver.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(BypassResult::Cancelled),
        }
    }
    
    /// non-blocking
    pub fn try_get_status(&self) -> Option<BrowserStatus> {
        match self.status_receiver.try_recv() {
            Ok(status) => Some(status),
            Err(_) => None,
        }
    }
    
    #[allow(dead_code)]
    pub fn cancel(&self) {
        if let Ok(mut flag) = self.cancel_flag.lock() {
            *flag = true;
        }
    }
}

async fn run_cloudflare_bypass_session(
    url: &str,
    result_sender: Sender<BypassResult>,
    status_sender: Sender<BrowserStatus>,
    cancel_flag: Arc<Mutex<bool>>,
) {
    use chromiumoxide::browser::{Browser, BrowserConfig};
    
    let _ = status_sender.send(BrowserStatus::Launching);
    
    tracing::info!("Launching browser for Cloudflare verification...");
    
    // visible so user can complete captcha
    let config = match BrowserConfig::builder()
        .window_size(1024, 768)
        .with_head()
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            let _ = result_sender.send(BypassResult::Error(
                format!("Failed to configure browser: {}. Make sure Chrome/Chromium is installed.", e)
            ));
            return;
        }
    };
    
    let (mut browser, mut handler) = match Browser::launch(config).await {
        Ok(b) => b,
        Err(e) => {
            let _ = result_sender.send(BypassResult::Error(
                format!("Failed to launch browser: {}. Make sure Chrome/Chromium is installed.", e)
            ));
            return;
        }
    };
    
    let handler_task = tokio::spawn(async move {
        while let Some(_event) = handler.next().await {}
    });
    
    let page = match browser.new_page(url).await {
        Ok(p) => p,
        Err(e) => {
            let _ = result_sender.send(BypassResult::Error(format!("Failed to open page: {}", e)));
            let _ = browser.close().await;
            handler_task.abort();
            return;
        }
    };
    
    let _ = status_sender.send(BrowserStatus::WaitingForVerification);
    tracing::info!("Browser opened. Waiting for Cloudflare verification...");
    
    let check_interval = std::time::Duration::from_millis(1500);
    let timeout = std::time::Duration::from_secs(120); // 2 minutes timeout
    let start = std::time::Instant::now();
    
    let mut cf_clearance_cookie: Option<String> = None;
    let mut verification_complete = false;
    let mut content_check_attempts = 0;
    
    loop {
        if let Ok(cancelled) = cancel_flag.lock() {
            if *cancelled {
                tracing::info!("Cloudflare bypass cancelled by user");
                let _ = result_sender.send(BypassResult::Cancelled);
                break;
            }
        }
        
        if start.elapsed() > timeout {
            tracing::warn!("Cloudflare bypass timed out after 2 minutes");
            let _ = result_sender.send(BypassResult::Error(
                "Verification timed out after 2 minutes. Please try again.".to_string()
            ));
            break;
        }
        
        if cf_clearance_cookie.is_none() {
            if let Ok(cookies) = page.get_cookies().await {
                for cookie in &cookies {
                    if cookie.name == "cf_clearance" && !cookie.value.is_empty() {
                        cf_clearance_cookie = Some(cookie.value.clone());
                        tracing::info!("Found cf_clearance cookie!");
                        break;
                    }
                }
            }
        }
        
        // check page content directly - cookie may already be set
        match page.content().await {
            Ok(content) => {
                let is_offset_content = content.contains("namespace") || 
                                        content.contains("constexpr") || 
                                        content.contains("uintptr_t") ||
                                        content.contains("inline constexpr");
                
                let is_challenge_page = content.contains("challenge-platform") || 
                                        content.contains("Just a moment") ||
                                        content.contains("Checking your browser") ||
                                        content.contains("cf-browser-verification");
                
                if is_offset_content && !is_challenge_page {
                    if !verification_complete {
                        verification_complete = true;
                        let _ = status_sender.send(BrowserStatus::Verified);
                        tracing::info!("Verification complete! Page content detected.");
                    }
                    
                    content_check_attempts += 1;
                    
                    // wait for page to stabilize before grabbing content
                    if content_check_attempts >= 2 {
                        let _ = status_sender.send(BrowserStatus::DownloadingContent);
                        tracing::info!("Downloading offset content...");
                        
                        let extracted = extract_code_content(&content);
                        
                        if extracted.contains("constexpr") || extracted.contains("namespace") || extracted.contains("0x") {
                            tracing::info!("Successfully retrieved offset content! ({} bytes)", extracted.len());
                            
                            if cf_clearance_cookie.is_none() {
                                if let Ok(cookies) = page.get_cookies().await {
                                    for cookie in &cookies {
                                        if cookie.name == "cf_clearance" && !cookie.value.is_empty() {
                                            cf_clearance_cookie = Some(cookie.value.clone());
                                            break;
                                        }
                                    }
                                }
                            }
                            
                            if let Some(ref cf) = cf_clearance_cookie {
                                if let Ok(parsed) = url::Url::parse(url) {
                                    if let Some(domain) = parsed.domain() {
                                        save_cookie_cache(cf, domain);
                                        tracing::info!("Saved cf_clearance cookie for future use");
                                    }
                                }
                            }
                            
                            let _ = result_sender.send(BypassResult::Success {
                                cf_clearance: cf_clearance_cookie.unwrap_or_default(),
                                content: extracted,
                            });
                            
                            let _ = browser.close().await;
                            handler_task.abort();
                            return;
                        } else {
                            tracing::debug!("Content extraction didn't yield valid data, retrying...");
                        }
                    }
                } else if is_challenge_page {
                    tracing::debug!("Still on Cloudflare challenge page...");
                    content_check_attempts = 0; // Reset counter
                } else {
                    if content.len() > 100 {
                        tracing::debug!("Page content unclear, length: {} bytes", content.len());
                    }
                }
            }
            Err(e) => {
                tracing::debug!("Failed to get page content: {}", e);
            }
        }
        
        tokio::time::sleep(check_interval).await;
    }
    
    // Cleanup
    let _ = browser.close().await;
    handler_task.abort();
}

fn extract_code_content(html: &str) -> String {
    // not HTML, return as-is
    if !html.contains("<html") && !html.contains("<!DOCTYPE") && !html.contains("<head") {
        return html.to_string();
    }
    
    // try <body> tag
    if let Some(body_start) = html.find("<body") {
        if let Some(body_content_start) = html[body_start..].find('>') {
            let content_start = body_start + body_content_start + 1;
            if let Some(body_end) = html[content_start..].find("</body>") {
                let body_content = &html[content_start..content_start + body_end];
                
                // try <pre> tag
                if let Some(pre_start) = body_content.find("<pre") {
                    if let Some(pre_content_start) = body_content[pre_start..].find('>') {
                        let pre_inner_start = pre_start + pre_content_start + 1;
                        if let Some(pre_end) = body_content[pre_inner_start..].find("</pre>") {
                            let content = &body_content[pre_inner_start..pre_inner_start + pre_end];
                            return decode_html_entities(content);
                        }
                    }
                }
                
                // Try <code> tag
                if let Some(code_start) = body_content.find("<code") {
                    if let Some(code_content_start) = body_content[code_start..].find('>') {
                        let code_inner_start = code_start + code_content_start + 1;
                        if let Some(code_end) = body_content[code_inner_start..].find("</code>") {
                            let content = &body_content[code_inner_start..code_inner_start + code_end];
                            return decode_html_entities(content);
                        }
                    }
                }
                
                // no pre/code, strip all HTML
                return strip_html_tags(body_content);
            }
        }
    }
    
    // fallback: <pre> anywhere
    if let Some(start) = html.find("<pre") {
        if let Some(end_tag_start) = html[start..].find('>') {
            let content_start = start + end_tag_start + 1;
            if let Some(end) = html[content_start..].find("</pre>") {
                let content = &html[content_start..content_start + end];
                return decode_html_entities(content);
            }
        }
    }
    
    // try <code> anywhere
    if let Some(start) = html.find("<code") {
        if let Some(end_tag_start) = html[start..].find('>') {
            let content_start = start + end_tag_start + 1;
            if let Some(end) = html[content_start..].find("</code>") {
                let content = &html[content_start..content_start + end];
                return decode_html_entities(content);
            }
        }
    }
    
    // last resort
    strip_html_tags(html)
}

fn decode_html_entities(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

fn strip_html_tags(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }
    
    decode_html_entities(&result)
}

pub async fn download_with_cloudflare_bypass(url: &str) -> Result<String> {
    tracing::info!("Starting Cloudflare bypass for: {}", url);
    
    if let Ok(parsed) = url::Url::parse(url) {
        if let Some(domain) = parsed.domain() {
            if let Some(cache) = load_cached_cookie(domain) {
                tracing::info!("Trying with cached cf_clearance cookie...");
                
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(15))
                    .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36")
                    .build()?;
                
                let response = client
                    .get(url)
                    .header("Cookie", format!("cf_clearance={}", cache.cf_clearance))
                    .header("Accept", "text/html,application/xhtml+xml,*/*")
                    .header("Accept-Language", "en-US,en;q=0.9")
                    .send()
                    .await?;
                
                if response.status().is_success() {
                    let content = response.text().await?;
                    
                    if content.contains("namespace") || content.contains("constexpr") {
                        tracing::info!("Cached cookie worked! Got valid content.");
                        return Ok(content);
                    }
                }
                
                tracing::info!("Cached cookie expired or invalid, starting browser...");
            }
        }
    }
    
    let session = CloudflareBypassSession::start(url)
        .map_err(|e| anyhow!("Failed to start browser session: {}", e))?;
    
    loop {
        while let Some(status) = session.try_get_status() {
            match status {
                BrowserStatus::Launching => tracing::info!("Launching browser..."),
                BrowserStatus::WaitingForVerification => {
                    tracing::info!("╔════════════════════════════════════════════════════════════╗");
                    tracing::info!("║     CLOUDFLARE VERIFICATION - Complete in Browser          ║");
                    tracing::info!("╠════════════════════════════════════════════════════════════╣");
                    tracing::info!("║  1. Complete the captcha in the browser window             ║");
                    tracing::info!("║  2. Wait for the page to load completely                   ║");
                    tracing::info!("║  3. The content will be downloaded automatically           ║");
                    tracing::info!("╚════════════════════════════════════════════════════════════╝");
                }
                BrowserStatus::Verified => tracing::info!("Verification complete! Downloading content..."),
                BrowserStatus::DownloadingContent => tracing::info!("Downloading offset data..."),
            }
        }
        
        if let Some(result) = session.try_get_result() {
            match result {
                BypassResult::Success { content, .. } => {
                    tracing::info!("Successfully downloaded content via browser!");
                    return Ok(content);
                }
                BypassResult::Cancelled => {
                    return Err(anyhow!("Cloudflare bypass was cancelled"));
                }
                BypassResult::Error(e) => {
                    return Err(anyhow!("Cloudflare bypass error: {}", e));
                }
            }
        }
        
        // avoid busy-waiting
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}
