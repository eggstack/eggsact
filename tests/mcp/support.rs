use serde_json::Value;

pub(crate) fn retry_mcp_request<F>(mut request_once: F) -> String
where
    F: FnMut() -> String,
{
    const MAX_ATTEMPTS: usize = 3;

    for attempt in 1..=MAX_ATTEMPTS {
        let response = request_once();
        if !response_needs_retry(&response) || attempt == MAX_ATTEMPTS {
            return response;
        }
        eprintln!(
            "MCP subprocess response was transient; retrying ({}/{})",
            attempt,
            MAX_ATTEMPTS - 1
        );
    }

    unreachable!("the retry loop always returns an MCP response")
}

fn response_needs_retry(response_str: &str) -> bool {
    let Ok(response) = serde_json::from_str::<Value>(response_str) else {
        return true;
    };
    let Some(result) = response.get("result") else {
        return false;
    };
    let Some(text) = result
        .get("content")
        .and_then(|content| content.as_array())
        .and_then(|content| content.first())
        .and_then(|first| first.get("text"))
        .and_then(|text| text.as_str())
    else {
        return true;
    };
    let Ok(envelope) = serde_json::from_str::<Value>(text) else {
        return true;
    };
    envelope
        .get("error_type")
        .and_then(|error_type| error_type.as_str())
        .is_some_and(|error_type| error_type == "timeout")
}
