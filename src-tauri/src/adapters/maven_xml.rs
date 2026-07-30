use quick_xml::events::{BytesText, Event};
use quick_xml::{Reader, Writer};

use crate::domain::{AdapterError, AdapterErrorCode};

pub fn replace_mirror_url(
    xml: &str,
    mirror_id: &str,
    next_url: &str,
) -> Result<String, AdapterError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut writer = Writer::new(Vec::new());
    let mut buffer = Vec::new();
    let mut inside_mirror = false;
    let mut current_tag = String::new();
    let mut current_mirror_id = String::new();
    let mut replaced = false;

    loop {
        let event = reader
            .read_event_into(&mut buffer)
            .map_err(|error| AdapterError {
                code: AdapterErrorCode::ParseFailure,
                message: format!("Maven XML 无法解析：{error}"),
            })?;
        match &event {
            Event::Start(start) => {
                current_tag = String::from_utf8_lossy(start.name().as_ref()).into_owned();
                if current_tag == "mirror" {
                    inside_mirror = true;
                    current_mirror_id.clear();
                }
                writer.write_event(event.to_owned()).map_err(write_error)?;
            }
            Event::Text(text) if inside_mirror && current_tag == "id" => {
                current_mirror_id = String::from_utf8_lossy(text.as_ref()).into_owned();
                writer.write_event(event.to_owned()).map_err(write_error)?;
            }
            Event::Text(_)
                if inside_mirror && current_tag == "url" && current_mirror_id == mirror_id =>
            {
                writer
                    .write_event(Event::Text(BytesText::new(next_url)))
                    .map_err(write_error)?;
                replaced = true;
            }
            Event::End(end) => {
                if end.name().as_ref() == b"mirror" {
                    inside_mirror = false;
                    current_mirror_id.clear();
                }
                current_tag.clear();
                writer.write_event(event.to_owned()).map_err(write_error)?;
            }
            Event::Eof => break,
            _ => writer.write_event(event.to_owned()).map_err(write_error)?,
        }
        buffer.clear();
    }

    if !replaced {
        return Err(AdapterError {
            code: AdapterErrorCode::InvalidInput,
            message: "未找到指定 Maven 镜像，无法生成变更。".into(),
        });
    }

    String::from_utf8(writer.into_inner()).map_err(|_| AdapterError {
        code: AdapterErrorCode::ParseFailure,
        message: "Maven XML 包含无效文本。".into(),
    })
}

fn write_error(error: std::io::Error) -> AdapterError {
    AdapterError {
        code: AdapterErrorCode::IoFailure,
        message: format!("无法写入 Maven XML：{error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_only_the_target_mirror_url_and_keeps_other_nodes() {
        let xml = r#"<?xml version="1.0"?><settings><!-- keep --><mirrors><mirror><id>central</id><url>https://old.example/</url><mirrorOf>*</mirrorOf></mirror><mirror><id>other</id><url>https://other.example/</url></mirror></mirrors><profiles><profile><id>keep</id></profile></profiles></settings>"#;
        let result = replace_mirror_url(xml, "central", "https://next.example/")
            .expect("target mirror should be replaced");

        assert!(result.contains("https://next.example/"));
        assert!(result.contains("https://other.example/"));
        assert!(result.contains("<!-- keep -->"));
        assert!(result.contains("<id>keep</id>"));
    }
}
