use clap::ValueEnum;
use snmp::{SnmpError, SyncSession, Value};
use strum::{AsRefStr, Display, EnumIter, EnumString, FromRepr, IntoStaticStr};

#[allow(non_camel_case_types)]
#[derive(Clone, Debug, ValueEnum, Copy, EnumIter, FromRepr, Display, IntoStaticStr, EnumString)]
#[strum(serialize_all = "kebab-case")]
#[repr(u32)]
pub enum Outlet {
    Unused1 = 1,
    W6otx_Allstar_rPi = 2,
    DishyPOE = 3,
    Switch = 4,
    Unused5 = 5,
    DMR_70cm = 6,
    FM_33cm = 7,
    DMR_2m = 8,
}

#[derive(Debug, FromRepr, Display, IntoStaticStr, AsRefStr)]
#[strum(serialize_all = "kebab-case")]
#[repr(i64)]
pub enum OutletStatus {
    Off = 1,
    On = 2,
}

#[derive(Debug, EnumString)]
#[strum(serialize_all = "kebab-case")]
#[repr(i64)]
pub enum OutletControlCommand {
    ImmediateOn = 1,
    ImmediateOff = 2,
    ImmediateReboot = 3,
    OutletUnknown = 4,
    DelayedOn = 5,
    DelayedOff = 6,
    DelayedReboot = 7,
    CancelPendingCommand = 8,
}

pub fn get_outlet_status(
    session: &mut SyncSession,
    outlet: Outlet,
) -> Result<OutletStatus, SnmpError> {
    let oid = make_outlet_status_oid(outlet as u32);
    let result = session.get(&oid);
    match result {
        Ok(mut pdu) => match pdu.varbinds.next() {
            Some((_, Value::Integer(n))) => {
                OutletStatus::from_repr(n).ok_or(SnmpError::ValueOutOfRange)
            }
            Some(_) | None => Err(SnmpError::ValueOutOfRange),
        },
        Err(error) => Err(error),
    }
}

pub fn control_outlet(
    session: &mut SyncSession,
    outlet: Outlet,
    command: OutletControlCommand,
) -> Result<(), SnmpError> {
    let oid = make_outlet_control_oid(outlet as u32);
    let value = Value::Integer(command as i64);
    session.set(&[(&oid, value)]).map(|_| ())
}

fn make_outlet_status_oid(n: u32) -> [u32; 16] {
    // iso.org.dod.internet.private.enterprises.apc.products
    // .hardware.rPDU2.rPDU2Outlet.rPDU2OutletSwitched
    // .rPDU2OutletSwitchedStatusTable.rPDU2OutletSwitchedStatusEntry
    // .rPDU2OutletSwitchedStatusState.n
    [1, 3, 6, 1, 4, 1, 318, 1, 1, 26, 9, 2, 3, 1, 5, n]
}

fn make_outlet_control_oid(n: u32) -> [u32; 16] {
    // .iso.org.dod.internet.private.enterprises.apc.products
    // .hardware.rPDU2.rPDU2Outlet.rPDU2OutletSwitched
    // .rPDU2OutletSwitchedControlTable.rPDU2OutletSwitchedControlEntry
    // .rPDU2OutletSwitchedControlCommand.n
    [1, 3, 6, 1, 4, 1, 318, 1, 1, 26, 9, 2, 4, 1, 5, n]
}
