mod transformer;

use crate::transformer::{process_array_convertible_objs, traverse_mut};
use anyhow::{anyhow, Result};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{to_string_pretty, to_value, Value};

/// Takes an input object and transform into an object that is the same structure as the passed output.
/// The output object's field values must contains the mapping details from the input object.
/// # Example
/// ```
///
/// use serde_json::json;
/// use transformer_rs::transform;
/// fn main() {
/// let input = json!({
///         "retailer": {
///             "id": "12342",
///         },
///
///         "order": {
///                 "po_number": "573832",
///                 "shipments": [
///                 {
///                     "tracking_number" : "1234567",
///                     "items": [
///                         {
///                             "sku": "SKU-123",
///                             "quantity": 4
///                         },
///                         {
///                             "sku": "SKU-343",
///                             "quantity": 3
///                         }
///                     ]
///                 },
///                 {
///                     "tracking_number" : "98776",
///                     "items": [
///                         {
///                             "sku": "SKU-1453",
///                             "quantity": 1
///                         },
///                         {
///                             "sku": "SKU-543",
///                             "quantity": 1
///                         }
///                     ]
///                 }
///             ],
///
///         },
///         "user_id": 2331212,
///         "order_id": "34554543",
///         "product": {
///             "id": "654654",
///             "length": 50,
///             "alternative_size": 33,
///             "details" :{
///                 "name": "Red Shoes",
///                 "manufacture": "company",
///             }
///         },
///         "ids": ["34554543", "7643534", "512342"]
///     });
///
///     let output = json!([
///         {
///           "order": {
///             "sub_order": {
///               "item_ids": "/ids",
///               "account_id": "/retailer/id",
///               "fulfillment_line_item_id": "/order/po_number",
///               "details": {
///                 ".trackings": "/order/shipments/tracking_number",
///                 "quantity": "/order/shipments/items/quantity"
///               },
///               "product": {
///                 "id": "/product/id"
///               }
///             }
///           }
///         }
///     ]);
///
///     let transformed_output = transform(&input, &output).unwrap();
///
///     println!(
///         "Output: {}",
///         serde_json::to_string_pretty(&transformed_output).unwrap()
///     );
///
/// }
/// ```
/// The transformed output should look like:
/// ```json
/// Output: [
///   "order": {
///     "sub_order": {
///       "account_id": "12342",
///       "details": {
///         ".trackings": [
///           "1234567",
///           "98776"
///         ],
///         "quantity": [
///           4,
///           3,
///           1,
///           1
///         ]
///       },
///       "fulfillment_line_item_id": "573832",
///       "product": {
///         "id": "654654"
///       },
///       "item_ids": [
///         "34554543",
///         "7643534",
///         "512342"
///       ]
///     }
///   }
/// ]
///```
/// It can map fields that are deeply nested inside array of arrays. In the example above the field
/// `quantity` in the output object has mapping `/order/shipments/items/quantity`, which is a field
/// inside an array, `shipments`, of arrays of objects, `items`.
///
/// # Concerting objects to array
/// Transform can also convert output object, or any descendant child object, into an array. In order to
/// do that, the object must have an array child which will be spread across the array of the objects.
/// Each object of the array, its original array field will be turned into a regular field instead of array
/// and that field will take one element from the original array. To enable this feature, a special field
/// decoration is required. To convert a specific object to array, it needs to be wrapped with square
/// brackets, i.g: `[order]`, also the child array to be spread should look like `...ids`.
/// Here is an example of an output json object:
/// ```json
/// [
///   {
///     "[order]": {
///       "sub_order": {
///         "...item_ids": "/ids",
///         "account_id": "/retailer/id",
///         "fulfillment_line_item_id": "/order/po_number",
///         "details": {
///           "trackings": "/order/shipments/tracking_number",
///           "quantity": "/order/shipments/items/quantity"
///         },
///         "product": {
///           "id": "/product/id"
///         }
///       }
///     }
///   }
/// ]
/// ```
/// The above result will be:
/// ```json
///   {
///     "order": [
///       {
///         "sub_order": {
///           "account_id": "12342",
///           "details": {
///             "quantity": [
///               4,
///               3,
///               1,
///               1
///             ],
///             "trackings": [
///               "1234567",
///               "98776"
///             ]
///           },
///           "fulfillment_line_item_id": "573832",
///           "item_ids": "34554543",
///           "product": {
///             "id": "654654"
///           }
///         }
///       },
///       {
///         "sub_order": {
///           "account_id": "12342",
///           "details": {
///             "quantity": [
///               4,
///               3,
///               1,
///               1
///             ],
///             "trackings": [
///               "1234567",
///               "98776"
///             ]
///           },
///           "fulfillment_line_item_id": "573832",
///           "item_ids": "7643534",
///           "product": {
///             "id": "654654"
///           }
///         }
///       },
///       {
///         "sub_order": {
///           "account_id": "12342",
///           "details": {
///             "quantity": [
///               4,
///               3,
///               1,
///               1
///             ],
///             "trackings": [
///               "1234567",
///               "98776"
///             ]
///           },
///           "fulfillment_line_item_id": "573832",
///           "item_ids": "512342",
///           "product": {
///             "id": "654654"
///           }
///         }
///       }
///     ]
///   }
///```
/// The spread array does not have to be an immediate child of the object that needs to be converted
/// into an array, it could be be something deeply nested. Example:
///
/// ```json
/// [
///   {
///     "[order]": {
///       "sub_order": {
///         "item_ids": "/ids",
///         "account_id": "/retailer/id",
///         "fulfillment_line_item_id": "/order/po_number",
///         "details": {
///           "trackings": "/order/shipments/tracking_number",
///           "...quantity": "/order/shipments/items/quantity"
///         },
///         "product": {
///           "id": "/product/id"
///         }
///       }
///     }
///   }
/// ]
/// ```
///
/// A multiple of objects with in the same obj, including the main one, can be converted to array.
/// Example:
///
/// ```json
/// [
///   {
///     "[order]": {
///       "sub_order": {
///         "...item_ids": "/ids",
///         "account_id": "/retailer/id",
///         "fulfillment_line_item_id": "/order/po_number",
///         "[details]": {
///           "...trackings": "/order/shipments/tracking_number",
///           "quantity": "/order/shipments/items/quantity"
///         },
///         "product": {
///           "id": "/product/id"
///         }
///       }
///     }
///   }
/// ]
/// ```
/// It is also possible to spread multiple arrays within the same object to converted to array,
/// as long as they are same length. In the example below, `details` will be converted to an array,
/// and each element will have one tracking and one quantity:
///
/// ```json
/// [
///   {
///     "[order]": {
///       "sub_order": {
///         "...item_ids": "/ids",
///         "account_id": "/retailer/id",
///         "fulfillment_line_item_id": "/order/po_number",
///         "[details]": {
///           "...trackings": "/order/shipments/tracking_number",
///           "...quantity": "/order/shipments/items/quantity"
///         },
///         "product": {
///           "id": "/product/id"
///         }
///       }
///     }
///   }
/// ]
/// ```
/// # Hard coded Values
/// Any field in the output object can be have hard coded value instead of mapping value. To hard code
/// a field value, simply use '', Example:
/// ```json
///  [
///    {
///      "product": {
///        "id": "'123345'"
///      }
///    }
///  ]
/// ```
pub fn transform<I, O>(input: &I, output: &O) -> Result<Value>
where
    I: Serialize + DeserializeOwned,
    O: Serialize + DeserializeOwned,
{
    let mut output: Value = to_value(output).unwrap();
    let input: Value = to_value(input).unwrap();

    let mut result: Vec<Value> = Vec::new();

    for mut obj in output
        .as_array_mut()
        .ok_or_else(|| anyhow!("output should be in an array of object structure"))?
        .iter_mut()
    {
        let string_pretty = to_string_pretty(&obj)?;
        let _obj_name = obj
            .as_object()
            .ok_or_else(|| {
                anyhow!(
                    "output array elements should be in object structure: {}",
                    string_pretty
                )
            })?
            .keys()
            .next()
            .ok_or_else(|| anyhow!("failed to get the name of the output: {}", string_pretty))?
            .clone();
        traverse_mut(&input, &mut obj, "", "")?;
        process_array_convertible_objs(
            &obj.clone(),
            &mut obj,
            "",
            "",
            &mut Default::default(),
            &mut Default::default(),
        )?;

        result.push(obj.clone());
    }

    Ok(to_value(result)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;
    use serde_json::{from_str, json, Value};
    use std::fs;
    use std::sync::Mutex;

    const OUTPUT_JSON_FILES_DIR: &str = "./test/output";
    static INPUT_JSON_FILE: Lazy<Mutex<Value>> = Lazy::new(|| {
        let input = fs::read_to_string("./test/input.json").expect("Unable to read input file");
        Mutex::new(from_str(&input).expect("Unable to parse input json file to value"))
    });

    #[test]
    fn transform_ok() {
        let output = fs::read_to_string(&format!("{}/default.json", OUTPUT_JSON_FILES_DIR))
            .expect("Unable to read file");
        let output: Value = from_str(&output).expect("Unable to parse input json file to value");

        let expected_transformed_output = fs::read_to_string(&format!(
            "{}/transformed/default.json",
            OUTPUT_JSON_FILES_DIR
        ))
        .expect(&format!(
            "Unable to read file {}/transformed/default.json",
            OUTPUT_JSON_FILES_DIR
        ));
        let expected_transformed_output: Value =
            from_str(&expected_transformed_output).expect(&format!(
                "Unable to parse file {}/transformed/default.json",
                OUTPUT_JSON_FILES_DIR
            ));

        let input = INPUT_JSON_FILE.lock().unwrap().clone();
        let transformed_output = transform(&input, &output);
        // println!(
        //     "Output: {}",
        //     to_string_pretty(&transformed_output.unwrap()).unwrap()
        // );
        assert!(transformed_output.is_ok());
        assert_eq!(transformed_output.unwrap(), expected_transformed_output);
    }

    #[test]
    fn transform_ok_hard_coded_value() {
        let output =
            fs::read_to_string(&format!("{}/hard_coded_value.json", OUTPUT_JSON_FILES_DIR))
                .expect("Unable to read file");
        let output: Value = from_str(&output).expect("Unable to parse input json file to value");

        let expected_transformed_output = fs::read_to_string(&format!(
            "{}/transformed/hard_coded_value.json",
            OUTPUT_JSON_FILES_DIR
        ))
        .expect(&format!(
            "Unable to read file {}/transformed/hard_coded_value.json",
            OUTPUT_JSON_FILES_DIR
        ));
        let expected_transformed_output: Value =
            from_str(&expected_transformed_output).expect(&format!(
                "Unable to parse file {}/transformed/hard_coded_value.json",
                OUTPUT_JSON_FILES_DIR
            ));

        let input = INPUT_JSON_FILE.lock().unwrap().clone();
        let transformed_output = transform(&input, &output);
        // println!(
        //     "Output: {}",
        //     to_string_pretty(&transformed_output.unwrap()).unwrap()
        // );
        assert!(transformed_output.is_ok());
        assert_eq!(transformed_output.unwrap(), expected_transformed_output);
    }

    #[test]
    fn transform_ok_object_to_array() {
        let output = fs::read_to_string(&format!("{}/array_obj.json", OUTPUT_JSON_FILES_DIR))
            .expect("Unable to read file");
        let output: Value = from_str(&output).expect("Unable to parse input json file to value");

        let expected_transformed_output = fs::read_to_string(&format!(
            "{}/transformed/array_obj.json",
            OUTPUT_JSON_FILES_DIR
        ))
        .expect(&format!(
            "Unable to read file {}/transformed/array_obj.json",
            OUTPUT_JSON_FILES_DIR
        ));
        let expected_transformed_output: Value =
            from_str(&expected_transformed_output).expect(&format!(
                "Unable to parse file {}/transformed/array_obj.json",
                OUTPUT_JSON_FILES_DIR
            ));

        let input = INPUT_JSON_FILE.lock().unwrap().clone();
        let transformed_output = transform(&input, &output);
        // println!(
        //     "Output: {}",
        //     to_string_pretty(&transformed_output.unwrap()).unwrap()
        // );
        assert!(transformed_output.is_ok());
        assert_eq!(transformed_output.unwrap(), expected_transformed_output);
    }

    #[test]
    fn transform_ok_object_to_array_2() {
        let output = fs::read_to_string(&format!("{}/array_obj_2.json", OUTPUT_JSON_FILES_DIR))
            .expect("Unable to read file");
        let output: Value = from_str(&output).expect("Unable to parse input json file to value");

        let expected_transformed_output = fs::read_to_string(&format!(
            "{}/transformed/array_obj_2.json",
            OUTPUT_JSON_FILES_DIR
        ))
        .expect(&format!(
            "Unable to read file {}/transformed/array_obj_2.json",
            OUTPUT_JSON_FILES_DIR
        ));
        let expected_transformed_output: Value =
            from_str(&expected_transformed_output).expect(&format!(
                "Unable to parse file {}/transformed/array_obj_2.json",
                OUTPUT_JSON_FILES_DIR
            ));

        let input = INPUT_JSON_FILE.lock().unwrap().clone();
        let transformed_output = transform(&input, &output);
        // println!(
        //     "Output: {}",
        //     to_string_pretty(&transformed_output.unwrap()).unwrap()
        // );
        assert!(transformed_output.is_ok());
        assert_eq!(transformed_output.unwrap(), expected_transformed_output);
    }

    #[test]
    fn transform_err_array_convertible_obj_no_spread_array_field() {
        let output = fs::read_to_string(&format!(
            "{}/bad_array_convertible_obj_structure.json",
            OUTPUT_JSON_FILES_DIR
        ))
        .expect("Unable to read file");
        let output: Value = from_str(&output).expect("Unable to parse input json file to value");

        let input = INPUT_JSON_FILE.lock().unwrap().clone();
        let transformed_output = transform(&input, &output);
        // println!(
        //     "Output: {}",
        //     to_string_pretty(&transformed_output.unwrap()).unwrap()
        // );
        assert!(transformed_output.is_err());
        assert_eq!(
            transformed_output.err().unwrap().to_string(),
            "Failed to process array convertible object; a array convertible object [order] is detected but no spread array field was found"
        );
    }

    #[test]
    fn transform_err_no_array_convertible_obj_and_spread_array_field() {
        let output = fs::read_to_string(&format!(
            "{}/bad_array_convertible_obj_structure_2.json",
            OUTPUT_JSON_FILES_DIR
        ))
        .expect("Unable to read file");
        let output: Value = from_str(&output).expect("Unable to parse input json file to value");

        let input = INPUT_JSON_FILE.lock().unwrap().clone();
        let transformed_output = transform(&input, &output);
        // println!(
        //     "Output: {}",
        //     to_string_pretty(&transformed_output.unwrap()).unwrap()
        // );
        assert!(transformed_output.is_ok());
    }

    #[test]
    fn transformer_bad_output_structure() {
        let input = INPUT_JSON_FILE.lock().unwrap().clone();
        let transformed_result = transform(&input, &json!({}));
        assert!(transformed_result.is_err());

        assert_eq!(
            transformed_result.err().unwrap().to_string(),
            "output should be in an array of object structure"
        );
    }

    #[test]
    fn transformer_bad_output_array_element_structure() {
        let input = INPUT_JSON_FILE.lock().unwrap().clone();
        let transformed_output = transform(&input, &json!([[]]));
        assert!(transformed_output.is_err());

        assert_eq!(
            transformed_output.err().unwrap().to_string(),
            "output array elements should be in object structure: []"
        );
    }

    #[test]
    fn transformer_output_array_obj_element_without_name() {
        let input = INPUT_JSON_FILE.lock().unwrap().clone();
        let transformed_output = transform(&input, &json!([{}]));
        assert!(transformed_output.is_err());

        assert_eq!(
            transformed_output.err().unwrap().to_string(),
            "failed to get the name of the output: {}"
        );
    }
}
