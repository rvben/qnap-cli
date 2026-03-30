use serde_json::Value;

pub fn print_value(value: &Value, json: bool) {
    if json {
        println!("{}", serde_json::to_string_pretty(value).unwrap_or_default());
    } else {
        print_table(value);
    }
}

fn print_table(value: &Value) {
    match value {
        Value::Object(map) => {
            let max_key = map.keys().map(|k| k.len()).max().unwrap_or(0);
            for (key, val) in map {
                let display = match val {
                    Value::String(s) => s.clone(),
                    Value::Null => "-".to_string(),
                    other => other.to_string(),
                };
                println!("  {:<width$}  {}", key, display, width = max_key);
            }
        }
        Value::Array(items) => {
            for item in items {
                print_table(item);
                println!();
            }
        }
        other => println!("{}", other),
    }
}
