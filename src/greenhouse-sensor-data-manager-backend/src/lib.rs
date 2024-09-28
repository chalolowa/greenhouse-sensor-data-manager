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
    updated_at: Option<u64>,
}

// Storable trait implementation for storing SensorData in stable storage.
impl Storable for SensorData {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

// BoundedStorable trait implementation for SensorData.
impl BoundedStorable for SensorData {
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    static ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
            .expect("Cannot create a counter")
    );

    static STORAGE: RefCell<StableBTreeMap<u64, SensorData, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1)))
    ));
}

// Payload structure for incoming sensor data.
#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct SensorDataPayload {
    device_id: String,
    temperature: f64,
    humidity: f64,
    soil_moisture: f64,
}

#[ic_cdk::query]
fn get_sensor_data(id: u64) -> Result<SensorData, Error> {
    match _get_sensor_data(&id) {
        Some(data) => Ok(data),
        None => Err(Error::NotFound {
            msg: format!("Sensor data with id={} not found", id),
        }),
    }
}

fn _get_sensor_data(id: &u64) -> Option<SensorData> {
    STORAGE.with(|s| s.borrow().get(id))
}

#[ic_cdk::update]
fn add_sensor_data(payload: SensorDataPayload) -> Option<SensorData> {
    let id = ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("cannot increment id counter");
    
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
    Some(data)
}

// Helper method to perform insert of sensor data into storage.
fn do_insert(data: &SensorData) {
    STORAGE.with(|service| service.borrow_mut().insert(data.id, data.clone()));
}

#[ic_cdk::update]
fn update_sensor_data(id: u64, payload: SensorDataPayload) -> Result<SensorData, Error> {
    match STORAGE.with(|service| service.borrow().get(&id)) {
        Some(mut data) => {
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
                "couldn't update sensor data with id={}. Data not found",
                id
            ),
        }),
    }
}

#[ic_cdk::update]
fn delete_sensor_data(id: u64) -> Result<SensorData, Error> {
    match STORAGE.with(|service| service.borrow_mut().remove(&id)) {
        Some(data) => Ok(data),
        None => Err(Error::NotFound {
            msg: format!(
                "couldn't delete sensor data with id={}. Data not found.",
                id
            ),
        }),
    }
}

#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },
}

// Generate candid
ic_cdk::export_candid!();
