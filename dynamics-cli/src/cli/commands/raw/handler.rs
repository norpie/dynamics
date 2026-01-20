//! Raw API command handler

use anyhow::{Context, Result};
use colored::*;
use std::fs;
use std::time::Instant;

use super::{DisplayStyle, HttpMethod, OutputFormat, RawCommands};

/// Handle the raw API command
pub async fn handle_raw_command(args: RawCommands) -> Result<()> {
    let client_manager = crate::client_manager();

    // Handle --no-color flag
    if args.no_color {
        colored::control::set_override(false);
    }

    // Determine environment
    let env_name = if let Some(ref env) = args.env {
        env.clone()
    } else {
        client_manager
            .get_current_environment()
            .await
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No environment selected. Use 'dynamics-cli auth env select' to choose one or specify --env."
                )
            })?
    };

    if matches!(args.style, DisplayStyle::Verbose) {
        println!("Using environment: {}", env_name.bright_green().bold());
        println!("Method: {}", format!("{:?}", args.method).bright_yellow());
        println!("Endpoint: {}", args.endpoint.cyan());
        if let Some(ref data) = args.data {
            println!("Data: {}", data.dimmed());
        }
        println!();
    }

    // Execute request
    let start_exec = Instant::now();

    if matches!(args.style, DisplayStyle::Verbose) {
        println!("Executing request...");
    }

    let client = client_manager.get_client(&env_name).await?;

    // Execute the raw API request
    let result = match args.method {
        HttpMethod::Get => client
            .execute_raw("GET", &args.endpoint, None)
            .await
            .context("Failed to execute GET request")?,
        HttpMethod::Post => {
            let data = args
                .data
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("POST request requires --data"))?;
            client
                .execute_raw("POST", &args.endpoint, Some(data))
                .await
                .context("Failed to execute POST request")?
        }
        HttpMethod::Patch => {
            let data = args
                .data
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("PATCH request requires --data"))?;
            client
                .execute_raw("PATCH", &args.endpoint, Some(data))
                .await
                .context("Failed to execute PATCH request")?
        }
        HttpMethod::Delete => client
            .execute_raw("DELETE", &args.endpoint, None)
            .await
            .context("Failed to execute DELETE request")?,
    };

    let exec_duration = start_exec.elapsed();

    if matches!(args.style, DisplayStyle::Verbose) {
        println!(
            "Execution time: {:.2}ms",
            exec_duration.as_secs_f64() * 1000.0
        );
        println!();
    }

    // Format and output results
    let formatted_output = format_output(&result, &args.format)?;

    if let Some(output_path) = args.output {
        fs::write(&output_path, &formatted_output)
            .with_context(|| format!("Failed to write output to: {}", output_path.display()))?;
        if matches!(args.style, DisplayStyle::Verbose) {
            println!(
                "Results saved to: {}",
                output_path.display().to_string().bright_green()
            );
        }
    } else {
        if matches!(args.style, DisplayStyle::Verbose) {
            println!("Results:");
            println!();
        }
        println!("{}", formatted_output);
    }

    Ok(())
}

/// Format API results according to the specified output format
fn format_output(data: &serde_json::Value, format: &OutputFormat) -> Result<String> {
    match format {
        OutputFormat::Json => {
            serde_json::to_string_pretty(data).context("Failed to format JSON output")
        }
        OutputFormat::JsonCompact => {
            serde_json::to_string(data).context("Failed to format JSON output")
        }
        OutputFormat::Xml => {
            // Convert JSON to XML representation
            let xml_string = json_to_xml(data)?;
            // Pretty-print XML
            match quick_xml::reader::Reader::from_str(&xml_string) {
                mut reader => {
                    let mut writer = quick_xml::Writer::new_with_indent(Vec::new(), b' ', 2);
                    let mut buf = Vec::new();
                    loop {
                        match reader.read_event_into(&mut buf) {
                            Ok(quick_xml::events::Event::Eof) => break,
                            Ok(event) => writer.write_event(event)?,
                            Err(_) => return Ok(xml_string), // fallback to raw
                        }
                        buf.clear();
                    }
                    Ok(String::from_utf8(writer.into_inner()).unwrap_or(xml_string))
                }
            }
        }
        OutputFormat::Csv => json_to_csv(data),
    }
}

/// Convert JSON data to XML representation
fn json_to_xml(data: &serde_json::Value) -> Result<String> {
    match data {
        serde_json::Value::Object(obj) => {
            let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<result>\n");
            for (key, value) in obj {
                xml.push_str(&format!(
                    "  <{}>{}</{}>\n",
                    key,
                    json_value_to_string(value),
                    key
                ));
            }
            xml.push_str("</result>");
            Ok(xml)
        }
        serde_json::Value::Array(arr) => {
            let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<results>\n");
            for (i, item) in arr.iter().enumerate() {
                xml.push_str(&format!(
                    "  <item index=\"{}\">{}</item>\n",
                    i,
                    json_value_to_string(item)
                ));
            }
            xml.push_str("</results>");
            Ok(xml)
        }
        _ => Ok(format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<value>{}</value>",
            json_value_to_string(data)
        )),
    }
}

/// Convert JSON data to CSV representation
fn json_to_csv(data: &serde_json::Value) -> Result<String> {
    match data {
        serde_json::Value::Array(arr) => {
            if arr.is_empty() {
                return Ok("No data\n".to_string());
            }

            let mut csv = String::new();

            // Extract headers from first object
            if let Some(serde_json::Value::Object(first_obj)) = arr.first() {
                let headers: Vec<String> = first_obj.keys().cloned().collect();
                csv.push_str(&headers.join(","));
                csv.push('\n');

                // Add data rows
                for item in arr {
                    if let serde_json::Value::Object(obj) = item {
                        let row: Vec<String> = headers
                            .iter()
                            .map(|h| {
                                csv_escape(&json_value_to_string(
                                    obj.get(h).unwrap_or(&serde_json::Value::Null),
                                ))
                            })
                            .collect();
                        csv.push_str(&row.join(","));
                        csv.push('\n');
                    }
                }
            }
            Ok(csv)
        }
        serde_json::Value::Object(obj) => {
            let mut csv = String::from("key,value\n");
            for (key, value) in obj {
                csv.push_str(&format!(
                    "{},{}\n",
                    csv_escape(key),
                    csv_escape(&json_value_to_string(value))
                ));
            }
            Ok(csv)
        }
        _ => Ok(format!(
            "value\n{}\n",
            csv_escape(&json_value_to_string(data))
        )),
    }
}

/// Convert a JSON value to a string representation
fn json_value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => value.to_string(),
    }
}

/// Escape a string for CSV output
fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}
