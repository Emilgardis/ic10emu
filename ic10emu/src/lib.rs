use core::f64;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

pub mod grammar;
pub mod interpreter;
mod rand_mscorlib;
pub mod tokens;

use grammar::{BatchMode, LogicType, ReagentMode, SlotLogicType};
use interpreter::{ICError, LineError};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug, Serialize, Deserialize)]
pub enum VMError {
    #[error("Device with id '{0}' does not exist")]
    UnknownId(u16),
    #[error("IC with id '{0}' does not exist")]
    UnknownIcId(u16),
    #[error("Device with id '{0}' does not have a IC Slot")]
    NoIC(u16),
    #[error("IC encountered an error: {0}")]
    ICError(#[from] ICError),
    #[error("IC encountered an error: {0}")]
    LineError(#[from] LineError),
    #[error("Invalid network ID {0}")]
    InvalidNetwork(u16),
    #[error("Device {0} not visible to device {1} (not on the same networks)")]
    DeviceNotVisible(u16, u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldType {
    Read,
    Write,
    ReadWrite,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogicField {
    pub field_type: FieldType,
    pub value: f64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Slot {
    pub typ: SlotType,
    // FIXME: this actualy needs to be an "Occupant" field
    // where the Occupant is an items with a
    // quantity, PrefabName/Hash, fields, etc
    pub fields: HashMap<grammar::SlotLogicType, LogicField>,
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub enum Connection {
    CableNetwork(Option<u16>),
    #[default]
    Other,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DeviceTemplate {
    pub name: Option<String>,
    pub hash: Option<i32>,
    pub logic: HashMap<grammar::LogicType, LogicField>,
    pub slots: Vec<SlotTemplate>,
    pub slotlogic: HashMap<grammar::LogicType, usize>,
    pub conn: Vec<(ConnectionType, ConnectionRole)>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionType {
    Pipe,
    Power,
    Data,
    Chute,
    Elevator,
    PipeLiquid,
    LandingPad,
    LaunchPad,
    PowerAndData,
    #[serde(other)]
    #[default]
    None,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionRole {
    Input,
    Input2,
    Output,
    Output2,
    Waste,
    #[serde(other)]
    #[default]
    None,
}

impl Connection {
    #[allow(dead_code)]
    fn from(typ: ConnectionType, _role: ConnectionRole) -> Self {
        match typ {
            ConnectionType::None
            | ConnectionType::Chute
            | ConnectionType::Pipe
            | ConnectionType::Elevator
            | ConnectionType::LandingPad
            | ConnectionType::LaunchPad
            | ConnectionType::PipeLiquid => Self::Other,
            ConnectionType::Data | ConnectionType::Power | ConnectionType::PowerAndData => {
                Self::CableNetwork(None)
            }
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlotType {
    AccessCard,
    Appliance,
    Back,
    Battery,
    Blocked,
    Bottle,
    Cartridge,
    Circuitboard,
    CreditCard,
    DataDisk,
    DrillHead,
    Egg,
    Entity,
    Flare,
    GasCanister,
    GasFilter,
    Helmet,
    Ingot,
    LiquidBottle,
    LiquidCanister,
    Magazine,
    Ore,
    Organ,
    Plant,
    ProgramableChip,
    ScanningHead,
    SensorProcessingUnit,
    SoundCartridge,
    Suit,
    Tool,
    Torpedo,
    #[default]
    #[serde(other)]
    None,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SlotTemplate {
    pub name: String,
    pub typ: SlotType,
}

#[derive(Debug, Default)]
pub struct Device {
    pub id: u16,
    pub name: Option<String>,
    pub name_hash: Option<f64>,
    pub fields: HashMap<grammar::LogicType, LogicField>,
    pub prefab_name: Option<String>,
    pub prefab_hash: Option<i32>,
    pub slots: Vec<Slot>,
    pub reagents: HashMap<ReagentMode, HashMap<i32, f64>>,
    pub ic: Option<u16>,
    pub connections: Vec<Connection>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Network {
    pub devices: HashSet<u16>,
    pub channels: [f64; 8],
}

#[derive(Debug, Default)]
struct IdSequenceGenerator {
    next: u16,
}

impl IdSequenceGenerator {
    pub fn next(&mut self) -> u16 {
        let val = self.next;
        self.next += 1;
        val
    }
}

#[derive(Debug)]
pub struct VM {
    pub ics: HashMap<u16, Rc<RefCell<interpreter::IC>>>,
    pub devices: HashMap<u16, Rc<RefCell<Device>>>,
    pub networks: HashMap<u16, Rc<RefCell<Network>>>,
    pub default_network: u16,
    id_gen: IdSequenceGenerator,
    network_id_gen: IdSequenceGenerator,
    random: Rc<RefCell<crate::rand_mscorlib::Random>>,

    /// list of device id's touched on the last operation
    operation_modified: RefCell<Vec<u16>>,
}

impl Default for Network {
    fn default() -> Self {
        Network {
            devices: HashSet::new(),
            channels: [f64::NAN; 8],
        }
    }
}

#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("")]
    ChannelIndexOutOfRange,
}

impl Network {
    pub fn contains(&self, ids: &[u16]) -> bool {
        ids.iter().all(|id| self.devices.contains(id))
    }

    pub fn add(&mut self, id: u16) -> bool {
        self.devices.insert(id)
    }

    pub fn remove(&mut self, id: u16) -> bool {
        self.devices.remove(&id)
    }

    pub fn set_channel(&mut self, chan: usize, val: f64) -> Result<f64, NetworkError> {
        if chan > 7 {
            Err(NetworkError::ChannelIndexOutOfRange)
        } else {
            let last = self.channels[chan];
            self.channels[chan] = val;
            Ok(last)
        }
    }

    pub fn get_channel(&self, chan: usize) -> Result<f64, NetworkError> {
        if chan > 7 {
            Err(NetworkError::ChannelIndexOutOfRange)
        } else {
            Ok(self.channels[chan])
        }
    }
}

impl Device {
    pub fn new(id: u16) -> Self {
        let mut device = Device {
            id,
            name: None,
            name_hash: None,
            prefab_name: None,
            prefab_hash: None,
            fields: HashMap::new(),
            slots: Vec::new(),
            reagents: HashMap::new(),
            ic: None,
            connections: vec![Connection::CableNetwork(None)],
        };
        device.fields.insert(
            LogicType::ReferenceId,
            LogicField {
                field_type: FieldType::Read,
                value: id as f64,
            },
        );
        device
    }

    pub fn with_ic(id: u16, ic: u16) -> Self {
        let mut device = Device::new(id);
        device.ic = Some(ic);
        device.connections = vec![Connection::CableNetwork(None), Connection::Other];
        device.prefab_name = Some("StructureCircuitHousing".to_owned());
        device.prefab_hash = Some(-128473777);
        device.fields.extend(vec![
            (
                LogicType::Power,
                LogicField {
                    field_type: FieldType::Read,
                    value: 1.0,
                },
            ),
            (
                LogicType::Error,
                LogicField {
                    field_type: FieldType::ReadWrite,
                    value: 0.0,
                },
            ),
            (
                LogicType::Setting,
                LogicField {
                    field_type: FieldType::ReadWrite,
                    value: 0.0,
                },
            ),
            (
                LogicType::On,
                LogicField {
                    field_type: FieldType::ReadWrite,
                    value: 0.0,
                },
            ),
            (
                LogicType::RequiredPower,
                LogicField {
                    field_type: FieldType::Read,
                    value: 0.0,
                },
            ),
            (
                LogicType::PrefabHash,
                LogicField {
                    field_type: FieldType::Read,
                    value: -128473777.0,
                },
            ),
            (
                LogicType::LineNumber,
                LogicField {
                    field_type: FieldType::ReadWrite,
                    value: 0.0,
                },
            ),
            (
                LogicType::ReferenceId,
                LogicField {
                    field_type: FieldType::Read,
                    value: id as f64,
                },
            ),
        ]);
        device.slots.push(Slot {
            typ: SlotType::ProgramableChip,
            fields: HashMap::from([
                (
                    SlotLogicType::PrefabHash,
                    LogicField {
                        field_type: FieldType::Read,
                        value: -744098481.0,
                    },
                ),
                (
                    SlotLogicType::LineNumber,
                    LogicField {
                        field_type: FieldType::Read,
                        value: 0.0,
                    },
                ),
            ]),
        });

        device
    }

    pub fn get_network_id(&self, connection: usize) -> Result<u16, ICError> {
        if connection >= self.connections.len() {
            Err(ICError::ConnectionIndexOutOfRange(
                connection,
                self.connections.len(),
            ))
        } else if let Connection::CableNetwork(network_id) = self.connections[connection] {
            if let Some(network_id) = network_id {
                Ok(network_id)
            } else {
                Err(ICError::NetworkNotConnected(connection))
            }
        } else {
            Err(ICError::NotDataConnection(connection))
        }
    }

    // FIXME: this needs some logic to link some special fields like "LineNumber" to the chip
    pub fn get_field(&self, typ: grammar::LogicType) -> Result<f64, ICError> {
        if let Some(field) = self.fields.get(&typ) {
            if field.field_type == FieldType::Read || field.field_type == FieldType::ReadWrite {
                Ok(field.value)
            } else {
                Err(ICError::WriteOnlyField(typ.to_string()))
            }
        } else {
            Err(ICError::DeviceHasNoField(typ.to_string()))
        }
    }

    // FIXME: this needs some logic to link some special fields like "LineNumber" to the chip
    pub fn set_field(&mut self, typ: grammar::LogicType, val: f64) -> Result<(), ICError> {
        if let Some(field) = self.fields.get_mut(&typ) {
            if field.field_type == FieldType::Write || field.field_type == FieldType::ReadWrite {
                field.value = val;
                Ok(())
            } else {
                Err(ICError::ReadOnlyField(typ.to_string()))
            }
        } else {
            Err(ICError::DeviceHasNoField(typ.to_string()))
        }
    }

    // FIXME: this needs to work with slot Occupants, see `Slot` decl
    pub fn get_slot_field(&self, index: f64, typ: grammar::SlotLogicType) -> Result<f64, ICError> {
        if let Some(field) = self
            .slots
            .get(index as usize)
            .ok_or(ICError::SlotIndexOutOfRange(index))?
            .fields
            .get(&typ)
        {
            if field.field_type == FieldType::Read || field.field_type == FieldType::ReadWrite {
                Ok(field.value)
            } else {
                Err(ICError::WriteOnlyField(typ.to_string()))
            }
        } else {
            Err(ICError::DeviceHasNoField(typ.to_string()))
        }
    }

    // FIXME: this needs to work with slot Occupants, see `Slot` decl
    pub fn set_slot_field(
        &mut self,
        index: f64,
        typ: grammar::SlotLogicType,
        val: f64,
    ) -> Result<(), ICError> {
        if let Some(field) = self
            .slots
            .get_mut(index as usize)
            .ok_or(ICError::SlotIndexOutOfRange(index))?
            .fields
            .get_mut(&typ)
        {
            if field.field_type == FieldType::Write || field.field_type == FieldType::ReadWrite {
                field.value = val;
                Ok(())
            } else {
                Err(ICError::ReadOnlyField(typ.to_string()))
            }
        } else {
            Err(ICError::DeviceHasNoField(typ.to_string()))
        }
    }

    pub fn get_reagent(&self, rm: &ReagentMode, reagent: f64) -> f64 {
        if let Some(mode) = self.reagents.get(rm) {
            if let Some(val) = mode.get(&(reagent as i32)) {
                return *val;
            }
        }
        0.0
    }

    pub fn set_name(&mut self, name: &str) {
        self.name_hash = Some((const_crc32::crc32(name.as_bytes()) as i32).into());
        self.name = Some(name.to_owned());
    }
}

impl Default for VM {
    fn default() -> Self {
        Self::new()
    }
}

impl VM {
    pub fn new() -> Self {
        let id_gen = IdSequenceGenerator::default();
        let mut network_id_gen = IdSequenceGenerator::default();
        let default_network = Rc::new(RefCell::new(Network::default()));
        let mut networks = HashMap::new();
        let default_network_key = network_id_gen.next();
        networks.insert(default_network_key, default_network);

        let mut vm = VM {
            ics: HashMap::new(),
            devices: HashMap::new(),
            networks,
            default_network: default_network_key,
            id_gen,
            network_id_gen,
            random: Rc::new(RefCell::new(crate::rand_mscorlib::Random::new())),
            operation_modified: RefCell::new(Vec::new()),
        };
        let _ = vm.add_ic(None);
        vm
    }

    fn new_device(&mut self) -> Device {
        Device::new(self.id_gen.next())
    }

    fn new_ic(&mut self) -> (Device, interpreter::IC) {
        let id = self.id_gen.next();
        let ic = interpreter::IC::new(id, id);
        let device = Device::with_ic(id, id);
        (device, ic)
    }

    pub fn add_device(&mut self, network: Option<u16>) -> Result<u16, VMError> {
        if let Some(n) = &network {
            if !self.networks.contains_key(n) {
                return Err(VMError::InvalidNetwork(*n));
            }
        }
        let mut device = self.new_device();
        if let Some(first_network) = device.connections.iter_mut().find_map(|c| {
            if let Connection::CableNetwork(c) = c {
                Some(c)
            } else {
                None
            }
        }) {
            first_network.replace(if let Some(network) = network {
                network
            } else {
                self.default_network
            });
        }
        let id = device.id;

        let first_data_network = device
            .connections
            .iter()
            .enumerate()
            .find_map(|(index, conn)| match conn {
                Connection::CableNetwork(_) => Some(index),
                Connection::Other => None,
            });
        self.devices.insert(id, Rc::new(RefCell::new(device)));
        if let Some(first_data_network) = first_data_network {
            let _ = self.add_device_to_network(
                id,
                if let Some(network) = network {
                    network
                } else {
                    self.default_network
                },
                first_data_network,
            );
        }
        Ok(id)
    }

    pub fn add_ic(&mut self, network: Option<u16>) -> Result<u16, VMError> {
        if let Some(n) = &network {
            if !self.networks.contains_key(n) {
                return Err(VMError::InvalidNetwork(*n));
            }
        }
        let (mut device, ic) = self.new_ic();
        if let Some(first_network) = device.connections.iter_mut().find_map(|c| {
            if let Connection::CableNetwork(c) = c {
                Some(c)
            } else {
                None
            }
        }) {
            first_network.replace(if let Some(network) = network {
                network
            } else {
                self.default_network
            });
        }
        let id = device.id;
        let ic_id = ic.id;
        let first_data_network = device
            .connections
            .iter()
            .enumerate()
            .find_map(|(index, conn)| match conn {
                Connection::CableNetwork(_) => Some(index),
                Connection::Other => None,
            });
        self.devices.insert(id, Rc::new(RefCell::new(device)));
        self.ics.insert(ic_id, Rc::new(RefCell::new(ic)));
        if let Some(first_data_network) = first_data_network {
            let _ = self.add_device_to_network(
                id,
                if let Some(network) = network {
                    network
                } else {
                    self.default_network
                },
                first_data_network,
            );
        }
        Ok(id)
    }

    pub fn add_network(&mut self) -> u16 {
        let next_id = self.network_id_gen.next();
        self.networks
            .insert(next_id, Rc::new(RefCell::new(Network::default())));
        next_id
    }

    pub fn get_default_network(&self) -> Rc<RefCell<Network>> {
        self.networks.get(&self.default_network).cloned().unwrap()
    }

    pub fn get_network(&self, id: u16) -> Option<Rc<RefCell<Network>>> {
        self.networks.get(&id).cloned()
    }

    pub fn remove_ic(&mut self, id: u16) {
        if self.ics.remove(&id).is_some() {
            self.devices.remove(&id);
        }
    }

    /// Set program code if it's valid
    pub fn set_code(&self, id: u16, code: &str) -> Result<bool, VMError> {
        let device = self
            .devices
            .get(&id)
            .ok_or(VMError::UnknownId(id))?
            .borrow();
        let ic_id = *device.ic.as_ref().ok_or(VMError::NoIC(id))?;
        let mut ic = self
            .ics
            .get(&ic_id)
            .ok_or(VMError::UnknownIcId(ic_id))?
            .borrow_mut();
        let new_prog = interpreter::Program::try_from_code(code)?;
        ic.program = new_prog;
        ic.ip = 0;
        ic.code = code.to_string();
        Ok(true)
    }

    /// Set program code and translate invalid lines to Nop, collecting errors
    pub fn set_code_invalid(&self, id: u16, code: &str) -> Result<bool, VMError> {
        let device = self
            .devices
            .get(&id)
            .ok_or(VMError::UnknownId(id))?
            .borrow();
        let ic_id = *device.ic.as_ref().ok_or(VMError::NoIC(id))?;
        let mut ic = self
            .ics
            .get(&ic_id)
            .ok_or(VMError::UnknownIcId(ic_id))?
            .borrow_mut();
        let new_prog = interpreter::Program::from_code_with_invalid(code);
        ic.program = new_prog;
        ic.ip = 0;
        ic.code = code.to_string();
        Ok(true)
    }

    /// returns a list of device ids modified in the last operations
    pub fn last_operation_modified(&self) -> Vec<u16> {
        self.operation_modified.borrow().clone()
    }

    pub fn step_ic(&self, id: u16, advance_ip_on_err: bool) -> Result<bool, VMError> {
        self.operation_modified.borrow_mut().clear();
        let device = self.devices.get(&id).ok_or(VMError::UnknownId(id))?.clone();
        let ic_id = *device.borrow().ic.as_ref().ok_or(VMError::NoIC(id))?;
        self.set_modified(id);
        let ic = self
            .ics
            .get(&ic_id)
            .ok_or(VMError::UnknownIcId(ic_id))?
            .clone();
        ic.borrow_mut().ic = 0;
        let result = ic.borrow_mut().step(self, advance_ip_on_err)?;
        Ok(result)
    }

    /// returns true if executed 128 lines, false if returned early.
    pub fn run_ic(&self, id: u16, ignore_errors: bool) -> Result<bool, VMError> {
        self.operation_modified.borrow_mut().clear();
        let device = self.devices.get(&id).ok_or(VMError::UnknownId(id))?.clone();
        let ic_id = *device.borrow().ic.as_ref().ok_or(VMError::NoIC(id))?;
        let ic = self
            .ics
            .get(&ic_id)
            .ok_or(VMError::UnknownIcId(ic_id))?
            .clone();
        ic.borrow_mut().ic = 0;
        self.set_modified(id);
        for _i in 0..128 {
            if let Err(err) = ic.borrow_mut().step(self, ignore_errors) {
                if !ignore_errors {
                    return Err(err.into());
                }
            }
            if let interpreter::ICState::Yield = ic.borrow().state {
                return Ok(false);
            } else if let interpreter::ICState::Sleep(_then, _sleep_for) = ic.borrow().state {
                return Ok(false);
            }
        }
        ic.borrow_mut().state = interpreter::ICState::Yield;
        Ok(true)
    }

    pub fn set_modified(&self, id: u16) {
        self.operation_modified.borrow_mut().push(id);
    }

    pub fn reset_ic(&self, id: u16) -> Result<bool, VMError> {
        let device = self.devices.get(&id).ok_or(VMError::UnknownId(id))?.clone();
        let ic_id = *device.borrow().ic.as_ref().ok_or(VMError::NoIC(id))?;
        let ic = self
            .ics
            .get(&ic_id)
            .ok_or(VMError::UnknownIcId(ic_id))?
            .clone();
        ic.borrow_mut().ic = 0;
        ic.borrow_mut().reset();
        Ok(true)
    }

    pub fn get_device(&self, id: u16) -> Option<Rc<RefCell<Device>>> {
        self.devices.get(&id).cloned()
    }

    pub fn batch_device(
        &self,
        source: u16,
        prefab_hash: f64,
        name: Option<f64>,
    ) -> impl Iterator<Item = &Rc<RefCell<Device>>> {
        self.devices
            .iter()
            .filter(move |(id, device)| {
                device
                    .borrow()
                    .fields
                    .get(&LogicType::PrefabHash)
                    .is_some_and(|f| f.value == prefab_hash)
                    && (name.is_none() || name == device.borrow().name_hash)
                    && self.devices_on_same_network(&[source, **id])
            })
            .map(|(_, d)| d)
    }

    pub fn get_device_same_network(&self, source: u16, other: u16) -> Option<Rc<RefCell<Device>>> {
        if self.devices_on_same_network(&[source, other]) {
            self.get_device(other)
        } else {
            None
        }
    }

    pub fn get_network_channel(&self, id: usize, channel: usize) -> Result<f64, ICError> {
        let network = self
            .networks
            .get(&(id as u16))
            .ok_or(ICError::BadNetworkId(id as u32))?;
        Ok(network.borrow().channels[channel])
    }

    pub fn set_network_channel(&self, id: usize, channel: usize, val: f64) -> Result<(), ICError> {
        let network = self
            .networks
            .get(&(id as u16))
            .ok_or(ICError::BadNetworkId(id as u32))?;
        network.borrow_mut().channels[channel] = val;
        Ok(())
    }

    pub fn devices_on_same_network(&self, ids: &[u16]) -> bool {
        for net in self.networks.values() {
            if net.borrow().contains(ids) {
                return true;
            }
        }
        false
    }

    /// return a vecter with the device ids the source id can see via it's connected netowrks
    pub fn visible_devices(&self, source: u16) -> Vec<u16> {
        self.networks
            .values()
            .filter_map(|net| {
                if net.borrow().contains(&[source]) {
                    Some(
                        net.borrow()
                            .devices
                            .iter()
                            .filter(|id| id != &&source)
                            .copied()
                            .collect_vec(),
                    )
                } else {
                    None
                }
            })
            .concat()
    }

    pub fn set_pin(&self, id: u16, pin: usize, val: Option<u16>) -> Result<bool, VMError> {
        let Some(device) = self.devices.get(&id) else {
            return Err(VMError::UnknownId(id));
        };
        if let Some(other_device) = val {
            if !self.devices.contains_key(&other_device) {
                return Err(VMError::UnknownId(other_device));
            }
            if !self.devices_on_same_network(&[id, other_device]) {
                return Err(VMError::DeviceNotVisible(other_device, id));
            }
        }
        if !(0..6).contains(&pin) {
            Err(ICError::PinIndexOutOfRange(pin).into())
        } else {
            let Some(ic_id) = device.borrow().ic else {
                return Err(VMError::NoIC(id));
            };
            self.ics.get(&ic_id).unwrap().borrow_mut().pins[pin] = val;
            Ok(true)
        }
    }

    pub fn add_device_to_network(
        &self,
        id: u16,
        network_id: u16,
        connection: usize,
    ) -> Result<bool, VMError> {
        if let Some(network) = self.networks.get(&network_id) {
            let Some(device) = self.devices.get(&id) else {
                return Err(VMError::UnknownId(id));
            };
            if connection >= device.borrow().connections.len() {
                let conn_len = device.borrow().connections.len();
                return Err(ICError::ConnectionIndexOutOfRange(connection, conn_len).into());
            }
            let Connection::CableNetwork(ref mut conn) =
                device.borrow_mut().connections[connection]
            else {
                return Err(ICError::NotDataConnection(connection).into());
            };
            *conn = Some(network_id);

            network.borrow_mut().add(id);
            Ok(true)
        } else {
            Err(VMError::InvalidNetwork(network_id))
        }
    }

    pub fn remove_device_from_network(&self, id: u16, network_id: u16) -> Result<bool, VMError> {
        if let Some(network) = self.networks.get(&network_id) {
            let Some(device) = self.devices.get(&id) else {
                return Err(VMError::UnknownId(id));
            };
            let mut device_ref = device.borrow_mut();

            for conn in device_ref.connections.iter_mut() {
                if let Connection::CableNetwork(conn) = conn {
                    if *conn == Some(network_id) {
                        *conn = None;
                    }
                }
            }
            network.borrow_mut().remove(id);
            Ok(true)
        } else {
            Err(VMError::InvalidNetwork(network_id))
        }
    }

    pub fn set_batch_device_field(
        &self,
        source: u16,
        prefab: f64,
        typ: LogicType,
        val: f64,
    ) -> Result<(), ICError> {
        self.batch_device(source, prefab, None)
            .map(|device| {
                self.set_modified(device.borrow().id);
                device.borrow_mut().set_field(typ, val)
            })
            .try_collect()
    }

    pub fn set_batch_device_slot_field(
        &self,
        source: u16,
        prefab: f64,
        index: f64,
        typ: SlotLogicType,
        val: f64,
    ) -> Result<(), ICError> {
        self.batch_device(source, prefab, None)
            .map(|device| {
                self.set_modified(device.borrow().id);
                device.borrow_mut().set_slot_field(index, typ, val)
            })
            .try_collect()
    }

    pub fn set_batch_name_device_field(
        &self,
        source: u16,
        prefab: f64,
        name: f64,
        typ: LogicType,
        val: f64,
    ) -> Result<(), ICError> {
        self.batch_device(source, prefab, Some(name))
            .map(|device| {
                self.set_modified(device.borrow().id);
                device.borrow_mut().set_field(typ, val)
            })
            .try_collect()
    }

    pub fn get_batch_device_field(
        &self,
        source: u16,
        prefab: f64,
        typ: LogicType,
        mode: BatchMode,
    ) -> Result<f64, ICError> {
        let samples = self
            .batch_device(source, prefab, None)
            .map(|device| device.borrow_mut().get_field(typ))
            .filter_ok(|val| !val.is_nan())
            .collect::<Result<Vec<_>, ICError>>()?;
        Ok(mode.apply(&samples))
    }

    pub fn get_batch_name_device_field(
        &self,
        source: u16,
        prefab: f64,
        name: f64,
        typ: LogicType,
        mode: BatchMode,
    ) -> Result<f64, ICError> {
        let samples = self
            .batch_device(source, prefab, Some(name))
            .map(|device| device.borrow_mut().get_field(typ))
            .filter_ok(|val| !val.is_nan())
            .collect::<Result<Vec<_>, ICError>>()?;
        Ok(mode.apply(&samples))
    }

    pub fn get_batch_name_device_slot_field(
        &self,
        source: u16,
        prefab: f64,
        name: f64,
        index: f64,
        typ: SlotLogicType,
        mode: BatchMode,
    ) -> Result<f64, ICError> {
        let samples = self
            .batch_device(source, prefab, Some(name))
            .map(|device| device.borrow().get_slot_field(index, typ))
            .filter_ok(|val| !val.is_nan())
            .collect::<Result<Vec<_>, ICError>>()?;
        Ok(mode.apply(&samples))
    }

    pub fn get_batch_device_slot_field(
        &self,
        source: u16,
        prefab: f64,
        index: f64,
        typ: SlotLogicType,
        mode: BatchMode,
    ) -> Result<f64, ICError> {
        let samples = self
            .batch_device(source, prefab, None)
            .map(|device| device.borrow().get_slot_field(index, typ))
            .filter_ok(|val| !val.is_nan())
            .collect::<Result<Vec<_>, ICError>>()?;
        Ok(mode.apply(&samples))
    }
}

impl BatchMode {
    pub fn apply(&self, samples: &[f64]) -> f64 {
        match self {
            BatchMode::Sum => samples.iter().sum(),
            BatchMode::Average => samples.iter().copied().sum::<f64>() / samples.len() as f64,
            BatchMode::Minimum => *samples
                .iter()
                .min_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap_or(&0.0),
            BatchMode::Maximum => *samples
                .iter()
                .max_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap_or(&0.0),
        }
    }
}
