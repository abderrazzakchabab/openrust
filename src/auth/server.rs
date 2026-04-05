use anyhow::Result;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Starts a local HTTP server on a random port.
/// Returns (port, future that resolves to the captured API key).
pub struct CallbackServer {
    listener: TcpListener,
    pub port: u16,
}

impl CallbackServer {
    pub async fn bind() -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        Ok(CallbackServer { listener, port })
    }

    /// Main serve loop — serves the form, then waits for POST with key.
    pub async fn serve(self, provider: &str) -> Result<String> {
        let form_html = build_form_page(provider, self.port);
        let success_html = build_success_page(provider);

        loop {
            let (stream, _) = self.listener.accept().await?;
            let req = read_request(stream).await?;

            match (req.method.as_str(), req.path.as_str()) {
                ("GET", "/") | ("GET", "") => {
                    write_response(req.stream, http_200(&form_html, "text/html; charset=utf-8"))
                        .await?;
                }
                ("POST", "/submit") => {
                    let key = parse_form_key(&req.body)
                        .unwrap_or_default()
                        .trim()
                        .to_string();
                    if !key.is_empty() {
                        write_response(
                            req.stream,
                            http_200(&success_html, "text/html; charset=utf-8"),
                        )
                        .await?;
                        return Ok(key);
                    }
                    // Empty key — redisplay form with error
                    let err_html = build_form_page_with_error(provider, self.port);
                    write_response(req.stream, http_200(&err_html, "text/html; charset=utf-8"))
                        .await?;
                }
                _ => {
                    write_response(req.stream, http_404()).await?;
                }
            }
        }
    }
}

struct ParsedRequest {
    method: String,
    path: String,
    body: String,
    stream: TcpStream,
}

async fn read_request(mut stream: TcpStream) -> Result<ParsedRequest> {
    let mut buf = vec![0u8; 8192];
    let n = stream.read(&mut buf).await?;
    let raw = String::from_utf8_lossy(&buf[..n]).to_string();

    let mut lines = raw.splitn(2, "\r\n\r\n");
    let headers_part = lines.next().unwrap_or("");
    let body = lines.next().unwrap_or("").to_string();

    let mut header_lines = headers_part.lines();
    let request_line = header_lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("GET").to_string();
    let path = parts.next().unwrap_or("/").to_string();

    Ok(ParsedRequest {
        method,
        path,
        body,
        stream,
    })
}

async fn write_response(mut stream: TcpStream, response: String) -> Result<()> {
    stream.write_all(response.as_bytes()).await?;
    stream.flush().await?;
    Ok(())
}

fn http_200(body: &str, content_type: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n{body}",
        len = body.len()
    )
}

fn http_404() -> String {
    let body = "Not Found";
    format!(
        "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
}

/// Parse `key=VALUE` from an `application/x-www-form-urlencoded` body.
fn parse_form_key(body: &str) -> Option<String> {
    for pair in body.split('&') {
        let mut kv = pair.splitn(2, '=');
        let k = kv.next()?;
        let v = kv.next().unwrap_or("");
        if k == "key" {
            // URL-decode basic percent-encoding and + → space
            let decoded = v.replace('+', " ");
            return Some(percent_decode(&decoded));
        }
    }
    None
}

fn percent_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    let bytes: Vec<u8> = s.bytes().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    result.push(byte as char);
                    i += 3;
                    continue;
                }
            }
        }
        result.push(bytes[i] as char);
        i += 1;
    }
    result
}

fn provider_display(provider: &str) -> &str {
    match provider {
        "anthropic" => "Anthropic (Claude)",
        "openai" => "OpenAI",
        _ => provider,
    }
}

fn build_form_page(provider: &str, _port: u16) -> String {
    build_form_page_inner(provider, "")
}

fn build_form_page_with_error(provider: &str, _port: u16) -> String {
    build_form_page_inner(
        provider,
        r#"<p class="error">API key cannot be empty. Please try again.</p>"#,
    )
}

fn build_form_page_inner(provider: &str, extra: &str) -> String {
    let display = provider_display(provider);
    let (key_hint, key_url) = match provider {
        "anthropic" => (
            "Starts with <code>sk-ant-</code>",
            "https://console.anthropic.com/settings/keys",
        ),
        "openai" => (
            "Starts with <code>sk-</code>",
            "https://platform.openai.com/api-keys",
        ),
        _ => ("Your API key", "#"),
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>OpenRust – {display} Login</title>
  <style>
    * {{ box-sizing: border-box; margin: 0; padding: 0; }}
    body {{
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
      background: #1e1e2e; color: #cdd6f4; min-height: 100vh;
      display: flex; align-items: center; justify-content: center;
    }}
    .card {{
      background: #181825; border: 1px solid #313244; border-radius: 12px;
      padding: 2.5rem; width: 100%; max-width: 520px; box-shadow: 0 8px 32px rgba(0,0,0,.4);
    }}
    .logo {{ font-size: 1.6rem; font-weight: 700; color: #89b4fa; margin-bottom: .25rem; }}
    .subtitle {{ color: #6c7086; font-size: .9rem; margin-bottom: 2rem; }}
    h2 {{ font-size: 1.15rem; color: #cdd6f4; margin-bottom: 1.25rem; }}
    label {{ display: block; font-size: .85rem; color: #a6adc8; margin-bottom: .4rem; }}
    input[type=text] {{
      width: 100%; padding: .75rem 1rem; background: #1e1e2e; border: 1px solid #313244;
      border-radius: 8px; color: #cdd6f4; font-size: .95rem; outline: none;
      font-family: monospace; transition: border-color .15s;
    }}
    input[type=text]:focus {{ border-color: #89b4fa; }}
    .hint {{ font-size: .8rem; color: #6c7086; margin-top: .4rem; }}
    .hint code {{ color: #a6e3a1; background: #1e1e2e; padding: .1rem .3rem; border-radius: 3px; }}
    button {{
      margin-top: 1.5rem; width: 100%; padding: .8rem; background: #89b4fa;
      color: #1e1e2e; border: none; border-radius: 8px; font-size: 1rem;
      font-weight: 600; cursor: pointer; transition: background .15s;
    }}
    button:hover {{ background: #b4d0f7; }}
    .link-row {{ margin-top: 1rem; text-align: center; font-size: .85rem; color: #6c7086; }}
    .link-row a {{ color: #89b4fa; text-decoration: none; }}
    .link-row a:hover {{ text-decoration: underline; }}
    .error {{ color: #f38ba8; font-size: .85rem; margin-top: .75rem; }}
    .step {{ background: #1e1e2e; border-radius: 8px; padding: .75rem 1rem;
             font-size: .85rem; color: #a6adc8; margin-bottom: 1.25rem; line-height: 1.6; }}
    .step strong {{ color: #89b4fa; }}
  </style>
</head>
<body>
  <div class="card">
    <div class="logo">OpenRust</div>
    <div class="subtitle">AI Terminal Coding Assistant</div>
    <h2>Connect {display}</h2>
    <div class="step">
      <strong>Step 1:</strong> Open your <a href="{key_url}" target="_blank" style="color:#89b4fa;">{display} API keys page</a>.<br>
      <strong>Step 2:</strong> Create a new key and copy it.<br>
      <strong>Step 3:</strong> Paste it below and click <em>Save</em>.
    </div>
    <form method="POST" action="/submit">
      <label for="key">API Key</label>
      <input type="text" id="key" name="key" placeholder="Paste your API key here…" autofocus autocomplete="off" spellcheck="false">
      <div class="hint">{key_hint}</div>
      {extra}
      <button type="submit">Save &amp; Continue</button>
    </form>
    <div class="link-row">
      Don't have a key? <a href="{key_url}" target="_blank">Get one here</a>
    </div>
  </div>
</body>
</html>"#
    )
}

fn build_success_page(provider: &str) -> String {
    let display = provider_display(provider);
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>OpenRust – Connected!</title>
  <style>
    * {{ box-sizing: border-box; margin: 0; padding: 0; }}
    body {{
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
      background: #1e1e2e; color: #cdd6f4; min-height: 100vh;
      display: flex; align-items: center; justify-content: center;
    }}
    .card {{
      background: #181825; border: 1px solid #313244; border-radius: 12px;
      padding: 2.5rem; width: 100%; max-width: 480px; text-align: center;
    }}
    .icon {{ font-size: 3rem; margin-bottom: 1rem; }}
    h2 {{ color: #a6e3a1; font-size: 1.4rem; margin-bottom: .75rem; }}
    p {{ color: #a6adc8; font-size: .95rem; line-height: 1.6; }}
    .close {{ margin-top: 1.5rem; font-size: .85rem; color: #6c7086; }}
  </style>
</head>
<body>
  <div class="card">
    <div class="icon">✓</div>
    <h2>{display} connected!</h2>
    <p>Your API key has been saved. You can close this tab and return to your terminal.</p>
    <div class="close">This window can be safely closed.</div>
  </div>
  <script>setTimeout(() => window.close(), 3000);</script>
</body>
</html>"#
    )
}
