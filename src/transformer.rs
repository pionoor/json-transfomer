use anyhow::{anyhow, bail, Error, Result};
use serde_json::{to_string_pretty, to_value, Value};
use std::collections::LinkedList;

// cleans key string from `...` or `[]`, example `...items` -> `item, `[order]` ->  `order`
fn clean_key(key: &str) -> Result<&str> {
    let mut clean_key = key;

    if is_obj_to_be_converted_to_array(key) {
        clean_key = key
            .strip_prefix('[')
            .map(|s| {
                s.strip_suffix(']').unwrap_or(key)
            })
            .ok_or_else(|| anyhow!(
                "Bad key format; array convertible objects notation should like \"[example_key]\": {}",
                key
            ))?;
    }
    if is_to_be_spread_array(key) {
        clean_key = key.strip_prefix("...").unwrap_or(key);
    }
    Ok(clean_key)
}

// cleans path from `...` or `[]`, example `[order]/...items/id` -> `order/items/id`
fn clean_path(path: &str) -> Result<String> {
    if path.is_empty() {
        return Ok(String::default());
    }
    let mut result = path.split('/').into_iter().collect::<Vec<&str>>();
    if result[0].is_empty() {
        result = result.drain(1..).collect();
    }
    let mut result = result.into_iter().try_fold("".to_string(), |xpath, key| {
        let mut cleaned_key = key;
        if is_obj_to_be_converted_to_array(key) || is_to_be_spread_array(key) {
            cleaned_key = clean_key(key)?
        }

        Ok::<String, Error>(format!("{}/{}", xpath, cleaned_key))
    })?;
    if result.is_empty() {
        result = clean_key(path)?.to_string();
    }
    Ok(result)
}

// Returns true if the object name wrapped in square brackets, example `[order]`
fn is_obj_to_be_converted_to_array(obj_name: &str) -> bool {
    obj_name.starts_with('[') && obj_name.ends_with(']')
}

// Returns true if the array name starts with 3 dots, example `...ids`
fn is_to_be_spread_array(array_name: &str) -> bool {
    array_name.contains("...")
}

// format keys by concatenating xpath and the key in the right format, example:
// xpath: "/order/items", key = "id" -> "/order/items/id"
fn format_key(xpath: &str, key: &str) -> String {
    match (xpath, key) {
        (x, "") => x.to_string(),
        ("", k) => {
            format!("/{}", k)
        }

        (x, k) => {
            format!("{}/{}", x, k)
        }
    }
}

// Treats input which is type of serde Value as tree. It uses depth first search algorithm for traversal
// It resolve the mapping value of each of the nodes and modifies it in place.
pub fn traverse_mut(input: &Value, output: &mut Value, xpath: &str, key: &str) -> Result<()> {
    match output {
        Value::Object(ref mut tree) => {
            for (sub_key, mut v) in tree.iter_mut() {
                traverse_mut(input, &mut v, &format_key(xpath, key), sub_key)?;
            }
            Ok(())
        }
        _ => {
            let output_field_value = output
                .as_str()
                .ok_or_else(|| {
                    anyhow!(
                        "Traversing output object failed; output object field should be string: {}",
                        output
                    )
                })?
                .to_owned();
            // check for hard coded values
            if output_field_value.starts_with('\'') && output_field_value.ends_with('\'') {
                *output = to_value(output_field_value.replace('\'', ""))?;
                return Ok(());
            }
            let mut path_tokens: LinkedList<&str> = output_field_value
                .split('/')
                .into_iter()
                .collect::<Vec<&str>>()
                .drain(1..) // gets rid of the unwanted "" from split
                .into_iter()
                .collect();
            *output = resolve_output_field_value(&mut path_tokens, input)?;
            Ok(())
        }
    }
}

// Takes mapping value. i.g "/order/shipments/items/quantity" and resolves it from the input object
// and returns the value.
pub fn resolve_output_field_value(
    path_tokens: &mut LinkedList<&str>,
    input: &Value,
) -> Result<Value> {
    let field_name = match path_tokens.pop_front() {
        None => {
            return Ok(input.clone());
        }
        Some(field_name) => field_name,
    };

    match input {
        Value::Array(array_values) => {
            let mut result_array = vec![];
            for element in array_values.iter() {
                let value = element.get(&field_name).ok_or(anyhow!(
                    "Failed to resolve mapping value; couldn't find field name {} in the obj {}",
                    &field_name,
                    to_string_pretty(&element)?
                ))?;
                if value.is_array() {
                    result_array.extend(value.as_array().unwrap());
                } else {
                    result_array.push(value);
                }
            }
            resolve_output_field_value(path_tokens, &to_value(result_array)?)
        }
        Value::Object(obj_value) => {
            return match obj_value.get(&field_name.to_owned()) {
                None => bail!(
                    "Failed to resolve mapping value; couldn't find field name {} in the obj {}",
                    &field_name,
                    to_string_pretty(&obj_value)?
                ),
                Some(field_value) => resolve_output_field_value(path_tokens, field_value),
            };
        }
        _ => Ok(input.clone()),
    }
}

// it traverse the transformed output and convert objects into arrays wherever found.
pub fn process_array_convertible_objs(
    input: &Value,
    output: &mut Value,
    xpath: &str,
    key: &str,
    visited: &mut LinkedList<String>,
    array_lens: &mut LinkedList<usize>,
) -> Result<()> {
    match input {
        Value::Object(ref tree) => {
            if is_obj_to_be_converted_to_array(key) {
                visited.push_back(format_key(&clean_path(xpath)?, clean_key(key)?));
                let parent_obj = if xpath.is_empty() {
                    output.as_object_mut().ok_or_else(|| {
                        anyhow!(
                        "Failed to process array convertible object; output expected to be object"
                    )
                    })?
                } else {
                    output
                        .pointer_mut(&clean_path(xpath)?)
                        .ok_or_else(||anyhow!("Failed to process array convertible object; failed for find parent object"))?
                        .as_object_mut()
                        .ok_or_else(|| anyhow!("Failed to process array convertible object; output expected to be object"))?
                };

                parent_obj.insert(
                    clean_key(key)?.to_string(),
                    parent_obj.get(key).ok_or_else(||anyhow!("Failed to process array convertible object; couldn't find field name {} in {:#?}", &key, &parent_obj))?.clone(),
                );
                parent_obj.remove(key).ok_or_else(|| anyhow!("Failed to process array convertible object; couldn't find field name {} in {:#?}", &key, &parent_obj))?;
            }

            for (sub_key, v) in tree.iter() {
                process_array_convertible_objs(
                    v,
                    output,
                    &format_key(xpath, key),
                    sub_key,
                    visited,
                    array_lens,
                )?;
            }

            // start array splitting
            if is_obj_to_be_converted_to_array(key) {
                split_obj_to_array(
                    output,
                    array_lens.pop_back().ok_or_else(|| anyhow!("Failed to process array convertible object; a array convertible object {} is detected but no spread array field was found", &key))?,
                    visited,
                    &clean_path(&format_key(xpath, key))?,
                    clean_key(key)?,
                )?
            }
        }
        _ => {
            if is_to_be_spread_array(key) {
                let parent_obj = output
                    .pointer_mut(&clean_path(xpath)?)
                    .ok_or_else(|| anyhow!("Failed to process array convertible object; unable to find the parent obj path of the array {}", &key))?
                    .as_object_mut()
                    .ok_or_else(|| anyhow!("Failed to process array convertible object; the parent obj of the spread array {} is not an object type", &key))?;
                parent_obj.insert(
                    clean_key(key)?.to_string(),
                    parent_obj.get(key).ok_or_else(||anyhow!("Failed to process array convertible object; couldn't find {} in {:#?} ", &key, &parent_obj))?.clone(),
                );
                parent_obj.remove(key).ok_or_else(|| {
                    anyhow!(
                    "Failed to process array convertible object; failed to remove {} from {:#?}",
                    &key,
                    &parent_obj
                )
                })?;
                if input.is_array() {
                    array_lens.push_back(
                        input
                            .as_array()
                            .ok_or_else(|| {
                                anyhow!(
                    "Failed to process array convertible object; failed to remove {} from {:#?}",
                    &key,
                    &parent_obj
                )
                            })?
                            .len(),
                    );
                }
                visited.push_back(format_key(&clean_path(xpath)?, clean_key(key)?));
            }
        }
    }
    Ok(())
}

// takes an object that contain the spread arrays and convert it into array of the same object, each
// takes one element from the array.
pub fn split_obj_to_array(
    output: &mut Value,
    array_len: usize,
    visited: &mut LinkedList<String>,
    path_to_array_parent_obj: &str,
    parent_obj_name: &str,
) -> Result<()> {
    // example: "/order/sub_order/details/trackings"
    let mut path_to_spread_array = visited.pop_back().ok_or_else(|| {
        anyhow!("Failed to split object to array; could not get path to the spread array")
    })?;
    let mut array_of_objs = vec![
        output
            .pointer(path_to_array_parent_obj)
            .ok_or_else(|| anyhow!(
                "Failed to split object to array; could not get path to the spread array"
            ))?
            .clone();
        array_len
    ];
    // path_to_array_parent_obj example: "/order/sub_order/details"
    while path_to_spread_array != path_to_array_parent_obj {
        // example "/tracking"
        let (_, array_path_from_parent_obj) = path_to_spread_array
            .split_once(parent_obj_name)
            .ok_or_else(|| {
                anyhow!("Failed to split object to array; could not get path to the spread array")
            })?;
        for (i, obj) in array_of_objs.iter_mut().enumerate() {
            let path = format_key(&path_to_spread_array, &i.to_string());
            let elem = output.pointer(&path).unwrap_or(&Value::Null).clone();
            let pretty_print_obj = to_string_pretty(&obj)?;
            *obj.pointer_mut(array_path_from_parent_obj).ok_or_else(|| {
                anyhow!(
                    "Failed to split object to array; could not find {} in {}",
                    &array_path_from_parent_obj,
                    &pretty_print_obj
                )
            })? = elem;
        }

        path_to_spread_array = visited.pop_back().ok_or_else(|| {
            anyhow!("Failed to split object to array; failed to get path token from the stack")
        })?;
    }
    *output
        .pointer_mut(path_to_array_parent_obj)
        .ok_or_else(|| anyhow!("Failed to split object to array; failed to get parent object of the spread array from output"))? =
        to_value(array_of_objs.clone())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;
    use serde_json::{json, Value};
    use std::fs;
    use std::sync::Mutex;

    const OUTPUT_JSON_FILES_DIR: &str = "./test/output";
    static INPUT_JSON_FILE: Lazy<Mutex<Value>> = Lazy::new(|| {
        let input = fs::read_to_string("./test/input.json").expect("Unable to read input file");
        Mutex::new(serde_json::from_str(&input).expect("Unable to parse input json file to value"))
    });

    #[test]
    fn test_clean_key() {
        assert_eq!(clean_key("").unwrap(), "");
        assert_eq!(clean_key("key").unwrap(), "key");
        assert_eq!(clean_key("...key").unwrap(), "key");
    }

    #[test]
    fn test_clean_path() {
        assert_eq!(clean_path("").unwrap(), "");
        assert_eq!(clean_path("/obj").unwrap(), "/obj");
        assert_eq!(clean_path("/obj/obj").unwrap(), "/obj/obj");
        assert_eq!(clean_path("obj/obj").unwrap(), "/obj/obj");
        assert_eq!(clean_path("/[obj]/...array").unwrap(), "/obj/array");
        assert_eq!(clean_path("/[obj]/obj/...array").unwrap(), "/obj/obj/array");
    }

    #[test]
    fn test_is_obj_to_be_converted_to_array() {
        assert_eq!(is_obj_to_be_converted_to_array("[obj]"), true);
        assert_eq!(is_obj_to_be_converted_to_array("obj"), false);
        assert_eq!(is_obj_to_be_converted_to_array("[obj"), false);
        assert_eq!(is_obj_to_be_converted_to_array("obj]"), false);
    }

    #[test]
    fn test_is_to_be_spread_array() {
        assert_eq!(is_to_be_spread_array("...array"), true);
        assert_eq!(is_to_be_spread_array("array"), false);
    }

    #[test]
    fn test_format_key() {
        assert_eq!(format_key("", ""), "");
        assert_eq!(format_key("", "key"), "/key");
        assert_eq!(format_key("/xpath/xpath", ""), "/xpath/xpath");
        assert_eq!(format_key("/xpath", "key"), "/xpath/key");
    }

    #[test]
    fn test_resolve_output_field_value_ok() {
        let input = INPUT_JSON_FILE.lock().unwrap().clone();

        let mut input_path_tokens: LinkedList<&str> = LinkedList::new();

        // regular field
        input_path_tokens.push_back("ids");
        let result = resolve_output_field_value(&mut input_path_tokens, &input);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            Value::from(vec!["34554543", "7643534", "512342"])
        );

        // nested field
        input_path_tokens.extend(["product", "details", "name"]);
        let result = resolve_output_field_value(&mut input_path_tokens, &input);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from("Red Shoes"));

        // field in an array
        input_path_tokens.extend(["order", "shipments", "tracking_number"]);
        let result = resolve_output_field_value(&mut input_path_tokens, &input);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from(vec!["1234567", "98776"]));

        // field in an array of arrays
        input_path_tokens.extend(["order", "shipments", "items"]);
        let result = resolve_output_field_value(&mut input_path_tokens, &input);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            Value::from(vec![
                json!({
                  "sku": "SKU-123",
                  "quantity": 4
                }),
                json!(    {
                  "sku": "SKU-343",
                  "quantity": 3
                }),
                json!({
                  "sku": "SKU-1453",
                  "quantity": 1
                }),
                json!({
                  "sku": "SKU-543",
                  "quantity": 1
                })
            ])
        );

        // field in an array of arrays of objs
        input_path_tokens.extend(["order", "shipments", "items", "sku"]);
        let result = resolve_output_field_value(&mut input_path_tokens, &input);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            Value::from(vec!["SKU-123", "SKU-343", "SKU-1453", "SKU-543"])
        );
    }

    #[test]
    fn test_resolve_output_field_value_err() {
        let input = INPUT_JSON_FILE.lock().unwrap().clone();

        let mut input_path_tokens: LinkedList<&str> = LinkedList::new();

        // field in an obj
        input_path_tokens.push_back("idsss");
        let result = resolve_output_field_value(&mut input_path_tokens, &input);
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap().to_string(),
            anyhow!(
                "Failed to resolve mapping value; couldn't find field name idsss in the obj {}",
                serde_json::to_string_pretty(&input).unwrap()
            )
            .to_string()
        );

        // field in an array of objs
        input_path_tokens.extend(["order", "shipments", "tracking_nomber"]);
        let result = resolve_output_field_value(&mut input_path_tokens, &input);
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap().to_string(),
            anyhow!(
                "Failed to resolve mapping value; couldn't find field name tracking_nomber in the obj {}",
                serde_json::to_string_pretty(&json!({
        "tracking_number" : "1234567",
        "items": [
          {
            "sku": "SKU-123",
            "quantity": 4
          },
          {
            "sku": "SKU-343",
            "quantity": 3
          }
        ]
      })).unwrap()).to_string());
    }

    #[test]
    fn test_traverse_mut_ok() {
        let input = INPUT_JSON_FILE.lock().unwrap().clone();
        let output = fs::read_to_string(&format!("{}/default.json", OUTPUT_JSON_FILES_DIR))
            .expect("Unable to read file");
        let mut output: Value =
            serde_json::from_str(&output).expect("Unable to parse input json file to value");

        let result = traverse_mut(&input, &mut output.get_mut(0).unwrap(), "", "");
        let expected_transformed_output = fs::read_to_string(&format!(
            "{}/transformed/default.json",
            OUTPUT_JSON_FILES_DIR
        ))
        .expect(&format!(
            "Unable to read file {}/transformed/default.json",
            OUTPUT_JSON_FILES_DIR
        ));
        let expected_transformed_output: Value = serde_json::from_str(&expected_transformed_output)
            .expect(&format!(
                "Unable to parse file {}/transformed/default.json",
                OUTPUT_JSON_FILES_DIR
            ));
        assert!(result.is_ok());
        assert_eq!(output, expected_transformed_output);
    }

    #[test]
    fn test_traverse_mut_err() {
        let input = INPUT_JSON_FILE.lock().unwrap().clone();

        let mut output = json!([[]]);
        let result = traverse_mut(&input, &mut output, "", "");

        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap().to_string(),
            "Traversing output object failed; output object field should be string: [[]]"
        )
    }
}
