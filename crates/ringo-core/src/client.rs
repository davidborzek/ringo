use anyhow::{Context, Result};
use serde_json::{Map, Value};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

#[derive(Debug)]
pub enum BaresipMessage {
    Event {
        class: String,
        type_: String,
        param: String,
        extra: Map<String, Value>,
    },
    Response {
        ok: bool,
        data: String,
        #[allow(dead_code)]
        token: Option<String>,
    },
}

/// Read one netstring-framed JSON message from `reader`.
pub async fn read_message<R: AsyncRead + Unpin>(reader: &mut R) -> Result<BaresipMessage> {
    // Read ASCII decimal length digits until ':'
    let mut len_bytes: Vec<u8> = Vec::new();
    loop {
        let mut b = [0u8; 1];
        reader
            .read_exact(&mut b)
            .await
            .context("Connection closed")?;
        if b[0] == b':' {
            break;
        }
        if !b[0].is_ascii_digit() {
            anyhow::bail!("Invalid netstring: expected digit, got 0x{:02x}", b[0]);
        }
        len_bytes.push(b[0]);
    }

    let len: usize = std::str::from_utf8(&len_bytes)
        .context("Invalid netstring length (UTF-8)")?
        .parse()
        .context("Invalid netstring length (parse)")?;

    // Read payload + trailing ','
    let mut payload = vec![0u8; len + 1];
    reader
        .read_exact(&mut payload)
        .await
        .context("Connection closed reading payload")?;

    if payload.last() != Some(&b',') {
        anyhow::bail!("Invalid netstring: missing trailing ','");
    }
    payload.pop();

    let json: Value = serde_json::from_slice(&payload).context("Invalid JSON in netstring")?;
    parse_message(json)
}

/// Encode `command` + `params` as a netstring and write to `writer`.
pub async fn write_command<W: AsyncWrite + Unpin>(
    writer: &mut W,
    command: &str,
    params: &str,
) -> Result<()> {
    let json = if params.is_empty() {
        serde_json::json!({"command": command})
    } else {
        serde_json::json!({"command": command, "params": params})
    };
    let json_str = json.to_string();
    let frame = format!("{}:{},", json_str.len(), json_str);
    writer
        .write_all(frame.as_bytes())
        .await
        .context("Failed to write command")?;
    writer.flush().await.context("Failed to flush")?;
    Ok(())
}

fn parse_message(json: Value) -> Result<BaresipMessage> {
    let obj = json.as_object().context("Expected JSON object")?;

    if obj.get("event").and_then(|v| v.as_bool()) == Some(true) {
        let class = obj
            .get("class")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let type_ = obj
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let param = obj
            .get("param")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let mut extra = obj.clone();
        extra.remove("event");
        extra.remove("class");
        extra.remove("type");
        extra.remove("param");
        return Ok(BaresipMessage::Event {
            class,
            type_,
            param,
            extra,
        });
    }

    if obj.get("response").and_then(|v| v.as_bool()) == Some(true) {
        let ok = obj.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
        let data = obj
            .get("data")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let token = obj
            .get("token")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        return Ok(BaresipMessage::Response { ok, data, token });
    }

    anyhow::bail!("Unknown message: missing 'event' or 'response' field")
}
