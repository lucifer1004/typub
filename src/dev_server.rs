//! Development server with live reload support.
//!
//! An HTTP server built on axum that serves preview HTML and
//! automatically refreshes the browser when content changes via SSE.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    extract::State,
    http::{StatusCode, Uri, header},
    response::{Html, Response, Sse},
    routing::get,
};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use typub_storage::mime_type_from_path;

/// Live reload script injected into HTML pages.
/// Uses SSE (Server-Sent Events) for efficient push-based updates.
const LIVE_RELOAD_SCRIPT: &str = r#"
<script>
(function() {
    let eventSource = null;
    let isConnected = false;

    // Scroll position management
    function saveScrollPosition() {
        return window.scrollY;
    }

    function restoreScrollPosition(y) {
        requestAnimationFrame(() => window.scrollTo(0, y));
    }

    // MathJax handling
    function waitForMathJax(maxAttempts = 50) {
        return new Promise((resolve) => {
            if (window.MathJax?.typesetPromise) return resolve();
            let attempts = 0;
            const check = setInterval(() => {
                attempts++;
                if (window.MathJax?.typesetPromise) {
                    clearInterval(check);
                    resolve();
                } else if (attempts >= maxAttempts) {
                    clearInterval(check);
                    resolve();
                }
            }, 100);
        });
    }

    function reextractLatexForMathJax() {
        document.querySelectorAll('[data-latex-src]').forEach(el => {
            const latex = el.getAttribute('data-latex-src');
            if (!latex) return;
            const isBlock = el.classList.contains('typst-svg-block');
            el.innerHTML = isBlock ? '$$' + latex + '$$' : '$' + latex + '$';
        });
    }

    async function renderMathJax() {
        if (!window.MathJax) return;

        reextractLatexForMathJax();

        try {
            if (window.MathJax.typesetPromise) {
                await MathJax.typesetPromise();
            } else if (window.MathJax.typeset) {
                MathJax.typeset();
            } else if (window.MathJax.Hub) {
                await new Promise(resolve => {
                    MathJax.Hub.Queue(['Typeset', MathJax.Hub, document.body]);
                    MathJax.Hub.Queue(resolve);
                });
            }
        } catch (e) {
            console.warn('[typub] MathJax error:', e);
        }
    }

    async function reloadContent() {
        const scrollY = saveScrollPosition();

        try {
            const response = await fetch('/');
            if (!response.ok) throw new Error('HTTP ' + response.status);

            const html = await response.text();
            const newDoc = new DOMParser().parseFromString(html, 'text/html');

            document.body.innerHTML = newDoc.body.innerHTML;

            await waitForMathJax();
            await renderMathJax();

            restoreScrollPosition(scrollY);
            console.log('[typub] Updated');
        } catch (e) {
            console.error('[typub] Update failed:', e);
        }
    }

    function connectSSE() {
        if (eventSource) {
            eventSource.close();
            eventSource = null;
        }

        eventSource = new EventSource('/__sse__');

        eventSource.onopen = () => {
            isConnected = true;
            console.log('[typub] Connected');
        };

        eventSource.onmessage = (event) => {
            console.log('[typub] Refresh:', event.data);
            reloadContent();
        };

        eventSource.onerror = () => {
            if (isConnected) {
                console.warn('[typub] Disconnected, reconnecting...');
                isConnected = false;
            }
            setTimeout(connectSSE, 1000);
        };
    }

    connectSSE();
})();
</script>
"#;

/// Shared state for the dev server
#[derive(Clone)]
struct DevServerState {
    /// Path to the HTML file being served
    html_path: PathBuf,
    /// Content root for resolving assets
    content_root: PathBuf,
    /// Directory containing the HTML file (for serving sibling assets like slide images)
    html_dir: PathBuf,
    /// Broadcast channel for SSE notifications
    tx: broadcast::Sender<()>,
}

/// Start a development server with live reload.
///
/// # Arguments
/// * `html_path` - Path to the HTML file to serve
/// * `content_root` - Path to content root for asset resolution
/// * `port` - Port to listen on (0 for random available port)
///
/// # Returns
/// The actual port, a shutdown function, and a notify function
pub fn start_dev_server(
    html_path: &std::path::Path,
    content_root: &std::path::Path,
    port: u16,
) -> Result<(u16, impl Fn() -> bool, impl Fn()), std::io::Error> {
    let html_path = html_path.to_path_buf();
    let content_root = content_root.to_path_buf();
    let html_dir = html_path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| content_root.clone());

    let (tx, _rx) = broadcast::channel::<()>(16);

    let state = Arc::new(DevServerState {
        html_path: html_path.clone(),
        content_root: content_root.clone(),
        html_dir: html_dir.clone(),
        tx: tx.clone(),
    });

    // Build router with tracing
    let app = Router::new()
        .route("/", get(serve_html))
        .route("/__sse__", get(serve_sse))
        .route("/__asset__/{*path}", get(serve_asset))
        .route("/{*path}", get(serve_preview_asset))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr: SocketAddr = if port == 0 {
        "127.0.0.1:0".parse().map_err(std::io::Error::other)?
    } else {
        format!("127.0.0.1:{}", port)
            .parse()
            .map_err(std::io::Error::other)?
    };

    let listener = std::net::TcpListener::bind(addr)?;
    let actual_port = listener.local_addr()?.port();
    listener.set_nonblocking(true)?;

    let tokio_listener = tokio::net::TcpListener::from_std(listener)?;

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);
    let shutdown_tx = Arc::new(std::sync::Mutex::new(Some(shutdown_tx)));

    tokio::spawn(async move {
        axum::serve(tokio_listener, app)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.recv().await;
            })
            .await
            .ok();
    });

    let shutdown_tx_clone = shutdown_tx.clone();
    let shutdown = move || {
        if let Ok(mut guard) = shutdown_tx_clone.lock()
            && let Some(tx) = guard.take()
        {
            let _ = tx.try_send(());
        }
        true
    };

    let tx_clone = tx;
    let notify = move || {
        let _ = tx_clone.send(());
    };

    Ok((actual_port, shutdown, notify))
}

/// Serve the main HTML file with live reload script injected
async fn serve_html(State(state): State<Arc<DevServerState>>) -> Result<Html<String>, StatusCode> {
    let original_html =
        std::fs::read_to_string(&state.html_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut html = original_html;

    if let Some(pos) = html.rfind("</body>") {
        html.insert_str(pos, LIVE_RELOAD_SCRIPT);
    } else {
        html.push_str(LIVE_RELOAD_SCRIPT);
    }

    Ok(Html(html))
}

/// Serve SSE for live reload
async fn serve_sse(
    State(state): State<Arc<DevServerState>>,
) -> Sse<
    impl tokio_stream::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>,
> {
    use axum::response::sse::Event;
    use tokio_stream::StreamExt;

    let rx = state.tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|result| match result {
        Ok(()) => Some(Ok(Event::default().data("reload"))),
        Err(_) => None,
    });

    Sse::new(stream)
}

/// Serve preview assets from /__asset__/
async fn serve_asset(
    State(state): State<Arc<DevServerState>>,
    uri: Uri,
) -> Result<Response<Body>, StatusCode> {
    let path = uri.path().trim_start_matches("/__asset__/");
    let decoded_path = urlencoding_decode(path);

    let mut file_path = state.content_root.clone();
    file_path.push(decoded_path);

    let content = std::fs::read(&file_path).map_err(|_| StatusCode::NOT_FOUND)?;
    let content_type = mime_type_from_path(&file_path);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, content.len())
        .body(Body::from(content))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Serve preview assets from the same directory as the HTML file.
/// This handles relative paths like slide images for xiaohongshu preview.
async fn serve_preview_asset(
    State(state): State<Arc<DevServerState>>,
    uri: Uri,
) -> Result<Response<Body>, StatusCode> {
    let path = uri.path().trim_start_matches('/');
    let decoded_path = urlencoding_decode(path);

    // Strip query string if present (e.g., "slide-1.png?v=123" -> "slide-1.png")
    let filename = decoded_path.split('?').next().unwrap_or(&decoded_path);

    // Only serve simple filenames, not paths with directories
    if filename.contains('/') || filename.is_empty() {
        return Err(StatusCode::NOT_FOUND);
    }

    let file_path = state.html_dir.join(filename);

    // Security check: ensure the resolved path is still within html_dir
    let canonical_path = file_path
        .canonicalize()
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let canonical_html_dir = state
        .html_dir
        .canonicalize()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !canonical_path.starts_with(&canonical_html_dir) {
        return Err(StatusCode::NOT_FOUND);
    }

    let content = std::fs::read(&file_path).map_err(|_| StatusCode::NOT_FOUND)?;
    let content_type = mime_type_from_path(&file_path);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, content.len())
        .header(header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
        .body(Body::from(content))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// URL decode for file paths
fn urlencoding_decode(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() == 2
                && let Ok(byte) = u8::from_str_radix(&hex, 16)
            {
                result.push(byte as char);
                continue;
            }
            result.push('%');
            result.push_str(&hex);
        } else {
            result.push(c);
        }
    }
    result
}
#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn test_urlencoding_decode() {
        assert_eq!(urlencoding_decode("hello"), "hello");
        assert_eq!(urlencoding_decode("hello%20world"), "hello world");
        assert_eq!(
            urlencoding_decode("/Users/test/file.png"),
            "/Users/test/file.png"
        );
    }
}
