/// This module defines the core data structures for an Asset Administration Shell (AAS).
/// Format and fields names are loosely based on the IDTA AAS specification available at
/// https://www.plattform-i40.de
use serde::{Deserialize, Serialize};
use serde_yaml;

use super::AssetID;

/// A top-level Asset Administration Shell (AAS).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetAdministrationShell {
    /// Unique identifier of the asset (URI, URN, or any unique string).
    pub id: AssetID,
    /// Human-readable name or short description.
    pub id_short: String,
    /// Optional: additional metadata about the asset or its owner.
    pub description: Option<String>,
    /// A set of Submodels describing various aspects of the asset.
    pub submodels: Vec<Submodel>,
}

/// A Submodel groups related data and operations about a particular aspect
/// of the asset (e.g., "Battery & Charging", "Maintenance", etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Submodel {
    /// Identifier for this submodel (could be a URN or any unique string).
    pub id: String,
    /// Human-readable identifier for the submodel.
    pub id_short: String,
    /// A collection of submodel elements, which might be properties, operations, events, etc.
    pub elements: Vec<SubmodelElement>,
}

/// A submodel element can be a property, an operation, an event, or
/// a composite element holding more elements. This enum captures those possibilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "element_type", rename_all = "lowercase")]
pub enum SubmodelElement {
    /// A Property holds a typed value (e.g., a string, integer, float).
    Property(Property),
    /// An Operation can define inputs, outputs, and the logic to invoke something.
    Operation(Operation),
    /// An Event can describe an asynchronous notification or trigger.
    Event(Event),
    /// A Collection can group multiple SubmodelElements hierarchically.
    Collection(SubmodelCollection),
    /// A reference to an external or internal resource (e.g., a sensor).
    ReferenceElement(ReferenceElement),
}

/// Represents a single property, such as "StateOfCharge", "Voltage", or "FirmwareVersion".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Property {
    pub id_short: String,
    pub value_type: ValueType,
    pub value: Value,
}

/// Represents an operation with (potential) inputs and outputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    pub id_short: String,
    /// Input variables: for example, a target charging current or a parameter for calibration.
    #[serde(default)]
    pub input_variables: Vec<OperationVariable>,
    /// Output variables: for example, a resulting status or a confirmation message.
    #[serde(default)]
    pub output_variables: Vec<OperationVariable>,
}

/// Represents an event, such as "ChargingStarted" or "LowBatteryAlert".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id_short: String,
    // Additional fields for event triggers, conditions, payload, etc.
}

/// A grouping of submodel elements. Useful to nest logical groups
/// like "BatterySensorData" containing multiple properties and events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmodelCollection {
    pub id_short: String,
    pub value: Vec<SubmodelElement>,
}

/// A reference element that points to an external entity, like an IoT sensor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceElement {
    pub id_short: String,
    /// This can be a URN, a URL, or an AAS-internal reference path.
    pub value: String,
}

/// A container for input/output parameters of an operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationVariable {
    pub name: String,
    /// This can store a "Property" or "DataSpecification" or just a typed value.
    pub value_type: ValueType,
    pub value: Value,
}

/// Simple enumeration for value types (string, integer, float, boolean, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueType {
    String,
    Int,
    Float,
    Bool,
    Json, // or more specialized, e.g. "Struct", "Array", etc.
}

/// A generic container for actual property values, to store
/// strongly typed fields or more sophisticated data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Str(String),
    Int(i64),
    Flt(f64),
    Bool(bool),
    Obj(serde_json::Value),
    Null,
}

impl AssetAdministrationShell {
    /// Load an AssetAdministrationShell from a YAML string.
    pub fn from_reader<R: std::io::Read>(reader: R) -> Result<Self, String> {
        // TODO: validate id_short with [a-zA-Z][a-zA-Z0-9_\-\.]{0,127}
        serde_yaml::from_reader(reader).map_err(|e| format!("Failed to parse YAML: {}", e))
    }

    /// Given a submodel ID, collection ID, and reference element ID,
    /// this method finds the reference element and returns its value.
    pub fn find_reference_value_in_collection(
        &self,
        submodel_id_short: &str,
        collection_id_short: &str,
        reference_element_id_short: &str,
    ) -> Option<String> {
        self.submodels
            .iter()
            .find(|s| s.id_short == submodel_id_short)
            .and_then(|submodel| {
                submodel.elements.iter().find_map(|elem| {
                    if let SubmodelElement::Collection(c) = elem {
                        if c.id_short == collection_id_short {
                            Some(c)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
            })
            .and_then(|collection| {
                collection.value.iter().find_map(|nested_elem| {
                    if let SubmodelElement::ReferenceElement(ref_elem) = nested_elem {
                        if ref_elem.id_short == reference_element_id_short {
                            Some(ref_elem.value.clone())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
            })
    }

    /// Resolve an AAS-style reference of the form:
    /// "urn:aas:smart-home:charging-station:datasources#SensorPowerAbsorption"
    /// and retrieve the "SensorID" property value from the referenced collection.
    pub fn resolve_sensor_reference(&self, full_ref: &str) -> Option<String> {
        let parts: Vec<&str> = full_ref.split('#').collect();
        if parts.len() != 2 {
            // Assuming references always have exactly one '#'
            return None;
        }
        let submodel_id = parts[0];
        let element_id_short = parts[1]; // e.g. "SensorPowerAbsorption"

        let submodel = self.submodels.iter().find(|s| s.id == submodel_id)?;
        let sensor_collection = submodel.elements.iter().find_map(|elem| {
            if let SubmodelElement::Collection(c) = elem {
                AssetAdministrationShell::find_collection_by_id_short(c, element_id_short)
            } else {
                None
            }
        })?;

        let sensor_id_prop = sensor_collection.value.iter().find_map(|elem| {
            if let SubmodelElement::Property(p) = elem {
                if p.id_short == "SensorID" {
                    Some(p.value.clone())
                } else {
                    None
                }
            } else {
                None
            }
        })?;

        // We expect sensor_id_prop to be a Value::Str("urn:iot-sensor:powerAbs123"), etc.
        if let Value::Str(sensor_id_str) = sensor_id_prop {
            Some(sensor_id_str)
        } else {
            None
        }
    }

    /// Recursively search for a sub-collection with the given id_short.
    pub fn find_collection_by_id_short(
        collection: &SubmodelCollection,
        target: &str,
    ) -> Option<SubmodelCollection> {
        if collection.id_short == target {
            return Some(collection.clone());
        }
        for elem in &collection.value {
            if let SubmodelElement::Collection(c) = elem {
                if let Some(found) = AssetAdministrationShell::find_collection_by_id_short(c, target) {
                    return Some(found);
                }
            }
        }
        None
    }

    /// Returns a list of all elements with the given ID found under the specified
    /// submodel (e.g., "IoTDataSources") and collection (e.g., "Sensors").
    /// - submodel: the short ID of the submodel containing the collection.
    /// - collection: the short ID of the collection containing the desired elements.
    /// - target: the short ID of the element to find (e.g., "SensorID").
    pub fn find_elements_in_collection(&self, submodel: &str, collection: &str, target: &str) -> Vec<String> {
        self.submodels
            .iter()
            .find(|s| s.id_short == submodel)
            .and_then(|submodel| {
                submodel.elements.iter().find_map(|elem| {
                    if let SubmodelElement::Collection(c) = elem {
                        if c.id_short == collection {
                            Some(c)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
            })
            .map_or_else(Vec::new, |sensors_collection| {
                let mut sensor_ids = Vec::new();
                AssetAdministrationShell::gather_sensor_ids_in_collection(
                    sensors_collection,
                    target,
                    &mut sensor_ids,
                );
                sensor_ids
            })
    }

    /// Recursively explores the given collection to find any sub-collections
    /// that contain a Property with id_short == "SensorID".
    fn gather_sensor_ids_in_collection(
        collection: &SubmodelCollection,
        target: &str,
        result: &mut Vec<String>,
    ) {
        for element in &collection.value {
            match element {
                SubmodelElement::Collection(sub_coll) => {
                    AssetAdministrationShell::gather_sensor_ids_in_collection(sub_coll, target, result);
                }
                SubmodelElement::Property(prop) => {
                    if prop.id_short == target {
                        // String expected
                        if let Value::Str(sensor_id) = &prop.value {
                            result.push(sensor_id.clone());
                        }
                    }
                }
                // Ignore other element types for the purpose of sensor IDs
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml;

    fn load_aas_from_yaml(yaml_str: &str) -> AssetAdministrationShell {
        serde_yaml::from_str(yaml_str).expect("Failed to parse YAML")
    }

    #[test]
    fn test_find_reference_value_in_collection() {
        let yaml = r#"
id: "urn:aas:example"
id_short: "ExampleAAS"
submodels:
  - id: "urn:aas:example:submodel1"
    id_short: "Submodel1"
    elements:
      - element_type: "collection"
        id_short: "Collection1"
        value:
        - element_type: "referenceelement"
          id_short: "Ref1"
          value: "http://example.com/resource"
"#;
        let aas = load_aas_from_yaml(yaml);

        let result = aas.find_reference_value_in_collection("Submodel1", "Collection1", "Ref1");
        assert_eq!(result, Some("http://example.com/resource".to_string()));
    }

    #[test]
    fn test_find_all_sensor_ids_in_datasources() {
        let yaml = r#"
id: "urn:aas:example"
id_short: "ExampleAAS"
submodels:
  - id: "urn:aas:example:submodel1"
    id_short: "IoTDataSources"
    elements:
      - element_type: "collection"
        id_short: "Sensors"
        value:
          - element_type: "property"
            id_short: "SensorID"
            value_type: "string"
            value: "Sensor123"
          - element_type: "collection"
            id_short: "NestedCollection"
            value:
              - element_type: "property"
                id_short: "SensorID"
                value_type: "string"
                value: "Sensor456"
"#;
        let aas = load_aas_from_yaml(yaml);

        let sensor_ids = aas.find_elements_in_collection("IoTDataSources", "Sensors", "SensorID");
        assert_eq!(sensor_ids, vec!["Sensor123".to_string(), "Sensor456".to_string()]);
    }

    #[test]
    fn test_resolve_sensor_reference() {
        let yaml = r#"
id: "urn:aas:example"
id_short: "ExampleAAS"
submodels:
  - id: "urn:aas:example:submodel1"
    id_short: "urn:aas:example:submodel1"
    elements:
      - element_type: "collection"
        id_short: "SensorPowerAbsorption"
        value:
          - element_type: "property"
            id_short: "SensorID"
            value_type: "string"
            value: "urn:iot-sensor:powerAbs123"
"#;
        let aas = load_aas_from_yaml(yaml);

        let sensor_id = aas.resolve_sensor_reference("urn:aas:example:submodel1#SensorPowerAbsorption");
        assert_eq!(sensor_id, Some("urn:iot-sensor:powerAbs123".to_string()));
    }

    #[test]
    fn test_find_collection_by_id_short() {
        let yaml = r#"
id: "urn:aas:example"
id_short: "ExampleAAS"
submodels:
  - id: "urn:aas:example:submodel1"
    id_short: "Submodel1"
    elements:
      - element_type: "collection"
        id_short: "ParentCollection"
        value:
          - element_type: "collection"
            id_short: "TargetCollection"
            value: []
"#;
        let aas = load_aas_from_yaml(yaml);

        let submodel = aas.submodels.iter().find(|s| s.id_short == "Submodel1").unwrap();
        let parent_collection = submodel
            .elements
            .iter()
            .find_map(|elem| {
                if let SubmodelElement::Collection(c) = elem {
                    Some(c)
                } else {
                    None
                }
            })
            .unwrap();

        let target_collection =
            AssetAdministrationShell::find_collection_by_id_short(parent_collection, "TargetCollection");
        assert!(target_collection.is_some());
        assert_eq!(target_collection.unwrap().id_short, "TargetCollection");
    }
}
