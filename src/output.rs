use serde_json::Value;

pub fn print_value(value: &Value, json: bool) {
    if json {
        println!("{}", serde_json::to_string_pretty(value).unwrap_or_default());
    } else {
        print_section(value, 0);
    }
}

fn print_section(value: &Value, depth: usize) {
    let indent = "  ".repeat(depth);
    match value {
        Value::Object(map) => {
            let max_key = map.keys().map(|k| k.len()).max().unwrap_or(0);
            for (key, val) in map {
                match val {
                    Value::Array(items) if !items.is_empty() && items[0].is_object() => {
                        println!("{}{}:", indent, key);
                        for item in items {
                            print_section(item, depth + 1);
                            println!();
                        }
                    }
                    Value::Object(_) => {
                        println!("{}{}:", indent, key);
                        print_section(val, depth + 1);
                    }
                    _ => {
                        let display = scalar_display(val);
                        println!(
                            "{}  {:<width$}  {}",
                            indent,
                            key,
                            display,
                            width = max_key
                        );
                    }
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                print_section(item, depth);
                println!();
            }
        }
        other => println!("{}{}", indent, other),
    }
}

fn scalar_display(val: &Value) -> String {
    match val {
        Value::String(s) => s.clone(),
        Value::Null => "-".to_string(),
        other => other.to_string(),
    }
}
