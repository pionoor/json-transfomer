## JSON Transformer
 Takes an input object and transform into an object that is the same structure as the passed output.
 The output object's field values must contains the mapping details from the input object.
 ### Example
 ```rust

 use serde_json::json;
 use transformer_rs::transform;
 fn main() {
 let input = json!({
         "retailer": {
             "id": "12342",
         },

         "order": {
                 "po_number": "573832",
                 "shipments": [
                 {
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
                 },
                 {
                     "tracking_number" : "98776",
                     "items": [
                         {
                             "sku": "SKU-1453",
                             "quantity": 1
                         },
                         {
                             "sku": "SKU-543",
                             "quantity": 1
                         }
                     ]
                 }
             ],

         },
         "user_id": 2331212,
         "order_id": "34554543",
         "product": {
             "id": "654654",
             "length": 50,
             "alternative_size": 33,
             "details" :{
                 "name": "Red Shoes",
                 "manufacture": "company",
             }
         },
         "ids": ["34554543", "7643534", "512342"]
     });

     let output = json!([
         {
           "order": {
             "sub_order": {
               "item_ids": "/ids",
               "account_id": "/retailer/id",
               "fulfillment_line_item_id": "/order/po_number",
               "details": {
                 ".trackings": "/order/shipments/tracking_number",
                 "quantity": "/order/shipments/items/quantity"
               },
               "product": {
                 "id": "/product/id"
               }
             }
           }
         }
     ]);

     let transformed_output = transform(&input, &output).unwrap();

     println!(
         "Output: {}",
         serde_json::to_string_pretty(&transformed_output).unwrap()
     );

 }
 ```
 The transformed output should look like:
 ```json
 Output:[
   "order": {
     "sub_order": {
       "account_id": "12342",
       "details": {
         ".trackings": [
           "1234567",
           "98776"
         ],
         "quantity": [
           4,
           3,
           1,
           1
         ]
       },
       "fulfillment_line_item_id": "573832",
       "product": {
         "id": "654654"
       },
       "item_ids": [
         "34554543",
         "7643534",
         "512342"
       ]
     }
   }
 ]
```
 It can map fields that are deeply nested inside array of arrays. In the example above the field
 `quantity` in the output object has mapping `/order/shipments/items/quantity`, which is a field
 inside an array, `shipments`, of arrays of objects, `items`.

 ### Concerting objects to array
 Transform can also convert output object, or any descendant child object, into an array. In order to
 do that, the object must have an array child which will be spread across the array of the objects.
 Each object of the array, its original array field will be turned into a regular field instead of array
 and that field will take one element from the original array. To enable this feature, a special field
 decoration is required. To convert a specific object to array, it needs to be wrapped with square
 brackets, i.g: `[order]`, also the child array to be spread should look like `...ids`.
 Here is an example of an output json object:
 ```json
 [
   {
     "[order]": {
       "sub_order": {
         "...item_ids": "/ids",
         "account_id": "/retailer/id",
         "fulfillment_line_item_id": "/order/po_number",
         "details": {
           "trackings": "/order/shipments/tracking_number",
           "quantity": "/order/shipments/items/quantity"
         },
         "product": {
           "id": "/product/id"
         }
       }
     }
   }
 ]
 ```
 The above result will be:
 ```json
   {
     "order": [
       {
         "sub_order": {
           "account_id": "12342",
           "details": {
             "quantity": [
               4,
               3,
               1,
               1
             ],
             "trackings": [
               "1234567",
               "98776"
             ]
           },
           "fulfillment_line_item_id": "573832",
           "item_ids": "34554543",
           "product": {
             "id": "654654"
           }
         }
       },
       {
         "sub_order": {
           "account_id": "12342",
           "details": {
             "quantity": [
               4,
               3,
               1,
               1
             ],
             "trackings": [
               "1234567",
               "98776"
             ]
           },
           "fulfillment_line_item_id": "573832",
           "item_ids": "7643534",
           "product": {
             "id": "654654"
           }
         }
       },
       {
         "sub_order": {
           "account_id": "12342",
           "details": {
             "quantity": [
               4,
               3,
               1,
               1
             ],
             "trackings": [
               "1234567",
               "98776"
             ]
           },
           "fulfillment_line_item_id": "573832",
           "item_ids": "512342",
           "product": {
             "id": "654654"
           }
         }
       }
     ]
   }
```
 The spread array does not have to be an immediate child of the object that needs to be converted
 into an array, it could be be something deeply nested. Example:

 ```json
 [
   {
     "[order]": {
       "sub_order": {
         "item_ids": "/ids",
         "account_id": "/retailer/id",
         "fulfillment_line_item_id": "/order/po_number",
         "details": {
           "trackings": "/order/shipments/tracking_number",
           "...quantity": "/order/shipments/items/quantity"
         },
         "product": {
           "id": "/product/id"
         }
       }
     }
   }
 ]
 ```

 A multiple of objects with in the same obj, including the main one, can be converted to array.
 Example:

 ```json
 [
   {
     "[order]": {
       "sub_order": {
         "...item_ids": "/ids",
         "account_id": "/retailer/id",
         "fulfillment_line_item_id": "/order/po_number",
         "[details]": {
           "...trackings": "/order/shipments/tracking_number",
           "quantity": "/order/shipments/items/quantity"
         },
         "product": {
           "id": "/product/id"
         }
       }
     }
   }
 ]
 ```
 It is also possible to spread multiple arrays within the same object to converted to array,
 as long as they are same length. In the example below, `details` will be converted to an array,
 and each element will have one tracking and one quantity:

 ```json
 [
   {
     "[order]": {
       "sub_order": {
         "...item_ids": "/ids",
         "account_id": "/retailer/id",
         "fulfillment_line_item_id": "/order/po_number",
         "[details]": {
           "...trackings": "/order/shipments/tracking_number",
           "...quantity": "/order/shipments/items/quantity"
         },
         "product": {
           "id": "/product/id"
         }
       }
     }
   }
 ]
 ```
 ### Hard coded Values
 Any field in the output object can be have hard coded value instead of mapping value. To hard code
 a field value, simply use `'EXAMPLE_HARD_CODED_VALUE'`, Example:
 ```json
 [
   {
     "product": {
       "id": "'123345'"
     }
   }
 ]
 ```
