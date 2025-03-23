/// This module defines the core data structures for an Asset Administration Shell (AAS).
/// Format and fields names are loosely based on the IDTA AAS specification available at
/// https://www.plattform-i40.de
use serde::{Deserialize, Serialize};

/// A top-level Asset Administration Shell (AAS).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetAdministrationShell {
    /// Unique identifier of the asset (URI, URN, or any unique string).
    pub id: String,
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
    // TODO In a real implementation, we could store the function/handler here
    // or reference a service endpoint that executes the operation.
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

    /// Returns a list of all sensor IDs found under the specified
    /// submodel (e.g., "IoTDataSources") and collection (e.g., "Sensors").
    pub fn find_all_sensor_ids_in_datasources(
        &self,
        submodel_id_short: &str,
        sensors_collection_id_short: &str,
    ) -> Vec<String> {
        self.submodels
            .iter()
            .find(|s| s.id_short == submodel_id_short)
            .and_then(|submodel| {
                submodel.elements.iter().find_map(|elem| {
                    if let SubmodelElement::Collection(c) = elem {
                        if c.id_short == sensors_collection_id_short {
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
                    &mut sensor_ids,
                );
                sensor_ids
            })
    }

    /// Recursively explores the given collection to find any sub-collections
    /// that contain a Property with id_short == "SensorID".
    fn gather_sensor_ids_in_collection(collection: &SubmodelCollection, result: &mut Vec<String>) {
        for element in &collection.value {
            match element {
                SubmodelElement::Collection(sub_coll) => {
                    AssetAdministrationShell::gather_sensor_ids_in_collection(sub_coll, result);
                }
                SubmodelElement::Property(prop) => {
                    if prop.id_short == "SensorID" {
                        // String expected for SensorID
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
