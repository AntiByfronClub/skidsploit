use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use base64::engine::general_purpose::STANDARD as base64Standard;
use base64::Engine as base64Engine;
use serde_json::{json, Value as jsonValue};
use tiny_http::{Method, Request, Response};

use crate::bytecode;
use crate::offsets;
use crate::rbx;
use crate::skid;

pub const bridgeAddr: (&str, u16) = ("127.0.0.1", 6767);

static attachedFlag: AtomicBool = AtomicBool::new(false);
static scriptQueue: Mutex<Vec<String>> = Mutex::new(Vec::new());
static clientPidStore: OnceLock<String> = OnceLock::new();
static webClientStore: OnceLock<reqwest::blocking::Client> = OnceLock::new();

fn webClient() -> &'static reqwest::blocking::Client {
    webClientStore.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .user_agent("skidsploit")
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("failed to build web client")
    })
}

const maxRedirects: usize = 20;

fn sendFollowRedirects(
    mut method: reqwest::Method,
    mut url: reqwest::Url,
    headerMap: reqwest::header::HeaderMap,
    mut payload: String,
) -> Result<reqwest::blocking::Response, bool> {
    let client = webClient();

    for _ in 0..=maxRedirects {
        let methodIsGet = method == reqwest::Method::GET;
        let mut builder = client
            .request(method.clone(), url.clone())
            .headers(headerMap.clone());
        if !methodIsGet {
            builder = builder.body(payload.clone());
        }

        let response = match builder.send() {
            Ok(value) => value,
            Err(error) => return Err(error.is_timeout()),
        };

        let status = response.status().as_u16();
        if !(300 < status && status < 400) {
            return Ok(response);
        }

        let location = match response
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.trim().to_string())
        {
            Some(value) => value,
            None => return Ok(response),
        };

        if location.is_empty() {
            return Ok(response);
        }

        let nextUrl = match url.join(&location) {
            Ok(value) => value,
            Err(_) => return Ok(response),
        };

        let scheme = nextUrl.scheme();
        if scheme != "http" && scheme != "https" {
            return Ok(response);
        }

        if status == 303 && !methodIsGet && method != reqwest::Method::HEAD {
            method = reqwest::Method::GET;
            payload = String::new();
        }

        url = nextUrl;
    }

    Err(false)
}

pub fn clientPid() -> String {
    clientPidStore.get().cloned().unwrap_or_default()
}

pub fn isAttached() -> bool {
    attachedFlag.load(Ordering::SeqCst)
}

pub fn markAttached(value: bool) {
    attachedFlag.store(value, Ordering::SeqCst);
}

pub fn requestExecution(script: String) {
    scriptQueue.lock().unwrap().push(script);
}

fn urlDecode(value: &str) -> String {
    let mut out: Vec<u8> = Vec::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let high = hexValue(bytes[index + 1]);
            let low = hexValue(bytes[index + 2]);
            out.push((high << 4) | low);
            index += 3;
        } else if bytes[index] == b'+' {
            out.push(b' ');
            index += 1;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }

    String::from_utf8_lossy(&out).into_owned()
}

fn hexValue(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => 0,
    }
}

fn parseQuery(query: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for pair in query.split('&') {
        let Some(eq) = pair.find('=') else {
            out.insert(urlDecode(pair), String::new());
            continue;
        };
        out.insert(urlDecode(&pair[..eq]), urlDecode(&pair[eq + 1..]));
    }
    out
}

fn queryParam(query: &str, key: &str) -> Option<String> {
    parseQuery(query)
        .get(key)
        .filter(|value| !value.is_empty())
        .cloned()
}

fn respond(request: Request, status: u16, body: &str) {
    let _ = request.respond(Response::from_string(body).with_status_code(status));
}

fn respondJson(request: Request, payload: &jsonValue) {
    let _ = request.respond(Response::from_data(payload.to_string().into_bytes()));
}

fn webErrorJson(status: &str, code: i32) -> jsonValue {
    json!({
        "b": "",
        "c": code,
        "r": status,
        "v": "HTTP/1.1",
        "h": {},
    })
}

fn writeBytecode(moduleName: &str, source: &str) -> Result<(), (u16, String)> {
    let compiled = bytecode::prepare(source, true)
        .map_err(|error| (500u16, format!("compile failed: {}", error)))?;

    let k = skid::skid();
    let fake = k.read::<usize>(k.baseAddr + offsets::datamodel::pointer);
    let datamodel = if fake != 0 {
        k.read::<usize>(fake + offsets::datamodel::real)
    } else {
        0
    };

    if datamodel == 0 {
        return Err((503, "no datamodel".to_string()));
    }

    let game = rbx::instance::new(datamodel);
    let container = game.find(&["CoreGui", "skid"]);
    if !container.nonZero() {
        return Err((404, "no container".to_string()));
    }

    let mut module = rbx::instance::default();
    for _ in 0..20 {
        module = container.findFirstChild(moduleName);
        if module.nonZero() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    if !module.nonZero() {
        return Err((404, format!("no module found with name: {}", moduleName)));
    }

    module.unlockModule();
    module.setBytecode(&compiled);

    Ok(())
}

fn handleLs(request: Request, query: &str) {
    if !isAttached() {
        respond(request, 400, "attach first");
        return;
    }

    let Some(moduleName) = queryParam(query, "c") else {
        respond(request, 400, "no c parameter");
        return;
    };

    let source = {
        let mut queue = scriptQueue.lock().unwrap();
        if queue.is_empty() {
            respond(request, 204, "");
            return;
        }
        queue.remove(0)
    };

    match writeBytecode(&moduleName, &source) {
        Ok(()) => respond(request, 200, "ok"),
        Err((status, body)) => respond(request, status, &body),
    }
}

fn handleExecute(request: Request, query: &str, body: &str) {
    if !isAttached() {
        respond(request, 400, "attach first");
        return;
    }

    let Some(pid) = queryParam(query, "pid") else {
        respond(request, 400, "no pid parameter");
        return;
    };

    if pid != clientPid() {
        respond(request, 403, "pid mismatch");
        return;
    }

    if body.is_empty() {
        respond(request, 400, "empty script");
        return;
    }

    requestExecution(body.to_string());
    respond(request, 200, "ok");
}

fn webHeaderMap(headers: &serde_json::Map<String, jsonValue>) -> reqwest::header::HeaderMap {
    let mut out = reqwest::header::HeaderMap::new();
    for (key, value) in headers {
        let (name, Some(value)) = (key.as_str(), value.as_str()) else {
            continue;
        };
        if let (Ok(name), Ok(value)) = (
            reqwest::header::HeaderName::from_bytes(name.as_bytes()),
            reqwest::header::HeaderValue::from_str(value),
        ) {
            out.insert(name, value);
        }
    }
    out
}

fn handleWebRequest(request: Request, body: &str) {
    let options: jsonValue = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(_) => {
            respondJson(request, &webErrorJson("Bad Request", 400));
            return;
        }
    };

    let url = options.get("l").and_then(|value| value.as_str()).unwrap_or("");
    let method = options.get("m").and_then(|value| value.as_str()).unwrap_or("").to_uppercase();
    let payload = options.get("b").and_then(|value| value.as_str()).unwrap_or("").to_string();

    let method = match method.as_str() {
        "GET" => reqwest::Method::GET,
        "POST" => reqwest::Method::POST,
        "PUT" => reqwest::Method::PUT,
        "DELETE" => reqwest::Method::DELETE,
        "PATCH" => reqwest::Method::PATCH,
        _ => {
            respondJson(request, &webErrorJson("Method Not Allowed", 405));
            return;
        }
    };

    let parsedUrl = match url.parse::<reqwest::Url>() {
        Ok(value) => value,
        Err(_) => {
            respondJson(request, &webErrorJson("Bad Request", 400));
            return;
        }
    };

    let methodIsGet = method == reqwest::Method::GET;

    let mut headerMap = match options.get("h").and_then(|value| value.as_object()) {
        Some(value) => webHeaderMap(value),
        None => reqwest::header::HeaderMap::new(),
    };

    if !methodIsGet && !headerMap.contains_key(reqwest::header::CONTENT_TYPE) {
        headerMap
            .insert(reqwest::header::CONTENT_TYPE, reqwest::header::HeaderValue::from_static("application/json"));
    }

    let response = match sendFollowRedirects(method, parsedUrl, headerMap, payload) {
        Ok(value) => value,
        Err(timedOut) => {
            if timedOut {
                respondJson(request, &webErrorJson("TimedOut", 0));
            } else {
                respondJson(request, &webErrorJson("ConnectFail", 0));
            }
            return;
        }
    };

    let statusCode = response.status().as_u16();
    let reason = response.status().canonical_reason().unwrap_or("Unknown").to_string();
    let version = match response.version() {
        reqwest::Version::HTTP_10 => "HTTP/1.0",
        reqwest::Version::HTTP_2 => "HTTP/2.0",
        reqwest::Version::HTTP_3 => "HTTP/3.0",
        _ => "HTTP/1.1",
    };

    let mut responseHeaders = serde_json::Map::new();
    for (name, value) in response.headers().iter() {
        if let Ok(text) = value.to_str() {
            responseHeaders.insert(name.as_str().to_string(), jsonValue::String(text.to_string()));
        }
    }

    let encoded = base64Standard.encode(response.bytes().unwrap_or_default());

    respondJson(
        request,
        &json!({
            "b": encoded,
            "c": statusCode,
            "r": reason,
            "v": version,
            "h": jsonValue::Object(responseHeaders),
        }),
    );
}

fn handleLoadstring(request: Request, query: &str, body: &str) {
    if !isAttached() {
        respond(request, 400, "attach first");
        return;
    }

    let Some(moduleName) = queryParam(query, "c") else {
        respond(request, 400, "no c parameter");
        return;
    };

    if body.is_empty() {
        respond(request, 400, "empty script");
        return;
    }

    match writeBytecode(&moduleName, &body) {
        Ok(()) => respond(request, 200, "ok"),
        Err((status, body)) => {
            respond(request, status, &body);
        }
    }
}

fn serve(bridge: tiny_http::Server) {
    for mut request in bridge.incoming_requests() {
        let url = request.url().to_string();
        let method = request.method().clone();

        let (path, query) = match url.split_once('?') {
            Some((path, query)) => (path.to_string(), query.to_string()),
            None => (url.clone(), String::new()),
        };

        let mut body = String::new();
        let _ = request.as_reader().read_to_string(&mut body);

        if method != Method::Post {
            respond(request, 404, "not found");
            continue;
        }

        match path.as_str() {
            "/ls" => handleLs(request, &query),
            "/loadstring" => handleLoadstring(request, &query, &body),
            "/execute" => handleExecute(request, &query, &body),
            "/request" => handleWebRequest(request, &body),
            _ => respond(request, 404, "not found"),
        }
    }
}

pub fn start(clientPid: String) {
    clientPidStore.set(clientPid).ok();

    let bridge = match tiny_http::Server::http(bridgeAddr) {
        Ok(value) => value,
        Err(error) => {
            crate::logErr!(
                "Bridge failed to start on {}:{}! ({})",
                bridgeAddr.0,
                bridgeAddr.1,
                error
            );
            std::process::exit(1);
        }
    };

    std::thread::spawn(move || serve(bridge));
}
