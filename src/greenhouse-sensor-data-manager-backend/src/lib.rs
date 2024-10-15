use serde::{Serialize, Deserialize};
use candid::{Decode, Encode};
use ic_cdk::api::time;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};

type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

// Struct to represent IoT data from a greenhouse sensor.
#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct SensorData {
    id: u64,
    device_id: String, // ID of the IoT device
    temperature: f64,
    humidity: f64,
    soil_moisture: f64,
    timestamp: u64,   // Timestamp of the data
    updated_at: Option<u64>, // Timestamp of last update
}

// Implementing Storable trait for SensorData for conversion to and from bytes for stable storage.
impl Storable for SensorData {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

// Implementing BoundedStorable trait for SensorData.
impl BoundedStorable for SensorData {
    const MAX_SIZE: u32 = 1024; // Max size for storing SensorData
    const IS_FIXED_SIZE: bool = false; // Flexible size since data can vary
}

thread_local! {
    // Memory Manager for stable memory operations
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    // ID counter for generating unique sensor data IDs
    static ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
            .expect("Cannot create a counter")
    );

    // Storage for SensorData using StableBTreeMap
    static STORAGE: RefCell<StableBTreeMap<u64, SensorData, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1)))
    ));
}

// Payload structure for incoming sensor data, used for creating or updating records.
#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct SensorDataPayload {
    device_id: String,
    temperature: f64,
    humidity: f64,
    soil_moisture: f64,
}

// Function to validate incoming sensor data payload before processing.
fn validate_sensor_data_payload(payload: &SensorDataPayload) -> Result<(), Error> {
    if payload.device_id.trim().is_empty() {
        return Err(Error::InvalidInput { msg: "Device ID cannot be empty".to_string() });
    }
    if !(0.0..=100.0).contains(&payload.humidity) {
        return Err(Error::InvalidInput { msg: "Humidity must be between 0 and 100".to_string() });
    }
    if payload.temperature < -50.0 || payload.temperature > 60.0 {
        return Err(Error::InvalidInput { msg: "Temperature must be between -50 and 60 degrees".to_string() });
    }
    if !(0.0..=100.0).contains(&payload.soil_moisture) {
        return Err(Error::InvalidInput { msg: "Soil moisture must be between 0 and 100".to_string() });
    }
    Ok(())
}

// Query to retrieve sensor data by ID.
#[ic_cdk::query]
fn get_sensor_data(id: u64) -> Result<SensorData, Error> {
    match _get_sensor_data(&id) {
        Some(data) => Ok(data),
        None => Err(Error::NotFound {
            msg: format!("Sensor data with id={} not found", id),
        }),
    }
}

// Internal helper function to get sensor data.
fn _get_sensor_data(id: &u64) -> Option<SensorData> {
    STORAGE.with(|s| s.borrow().get(id))
}

// Update function to add new sensor data.
#[ic_cdk::update]
fn add_sensor_data(payload: SensorDataPayload) -> Result<SensorData, Error> {
    // Validate the incoming sensor data payload.
    validate_sensor_data_payload(&payload)?;

    let id = ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("Cannot increment ID counter");

    let data = SensorData {
        id,
        device_id: payload.device_id,
        temperature: payload.temperature,
        humidity: payload.humidity,
        soil_moisture: payload.soil_moisture,
        timestamp: time(),
        updated_at: None,
    };

    do_insert(&data);
    Ok(data)
}

// Helper method to insert sensor data into stable storage.
fn do_insert(data: &SensorData) {
    STORAGE.with(|service| service.borrow_mut().insert(data.id, data.clone()));
}

// Update function to modify existing sensor data by ID.
#[ic_cdk::update]
fn update_sensor_data(id: u64, payload: SensorDataPayload) -> Result<SensorData, Error> {
    // Validate the payload before updating.
    validate_sensor_data_payload(&payload)?;

    match STORAGE.with(|service| service.borrow().get(&id)) {
        Some(mut data) => {
            // Update sensor data fields with new values.
            data.device_id = payload.device_id;
            data.temperature = payload.temperature;
            data.humidity = payload.humidity;
            data.soil_moisture = payload.soil_moisture;
            data.updated_at = Some(time());
            do_insert(&data);
            Ok(data)
        }
        None => Err(Error::NotFound {
            msg: format!(
                "Couldn't update sensor data with id={}. Data not found",
                id
            ),
        }),
    }
}

// Function to delete sensor data by ID.
#[ic_cdk::update]
fn delete_sensor_data(id: u64) -> Result<SensorData, Error> {
    match STORAGE.with(|service| service.borrow_mut().remove(&id)) {
        Some(data) => Ok(data),
        None => Err(Error::NotFound {
            msg: format!(
                "Couldn't delete sensor data with id={}. Data not found.",
                id
            ),
        }),
    }
}

// Custom error types for validation and not found cases.
#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },  // Error type for when sensor data isn't found
    InvalidInput { msg: String },  // Error type for invalid input during validation
}

// Generate candid interface for the code.
ic_cdk::export_candid!();
