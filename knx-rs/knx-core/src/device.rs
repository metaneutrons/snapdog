// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Fabian Schmieder

//! Device management types — restart, erase, security, and return codes.

/// Restart type for restart service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RestartType {
    /// Basic restart.
    Basic = 0x00,
    /// Master reset.
    MasterReset = 0x01,
}

/// Erase code for master reset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EraseCode {
    /// No erase.
    Void = 0x00,
    /// Confirmed restart.
    ConfirmedRestart = 0x01,
    /// Factory reset (all data).
    FactoryReset = 0x02,
    /// Reset individual address only.
    ResetIndividualAddress = 0x03,
    /// Reset application program.
    ResetApplicationProgram = 0x04,
    /// Reset parameters.
    ResetParameters = 0x05,
    /// Reset links (group address associations).
    ResetLinks = 0x06,
    /// Factory reset without changing individual address.
    FactoryResetWithoutAddress = 0x07,
}

/// Data security level for KNX Data Secure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataSecurity {
    /// No security.
    None,
    /// Authentication only.
    Auth,
    /// Authentication and confidentiality.
    AuthConf,
}

/// Security control information attached to secured telegrams.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SecurityControl {
    /// Whether this is a tool access (ETS programming).
    pub tool_access: bool,
    /// The data security level.
    pub data_security: DataSecurity,
}

/// Unified return codes for KNX services and functions.
///
/// Note: several older KNX services do not use these return codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ReturnCode {
    // ── Positive ──────────────────────────────────────────────
    /// Service executed successfully.
    Success = 0x00,
    /// Positive confirmation with CRC over original data.
    SuccessWithCrc = 0x01,

    // ── Negative ──────────────────────────────────────────────
    /// Memory cannot be accessed or only with faults.
    MemoryError = 0xF1,
    /// Server does not support the requested command.
    InvalidCommand = 0xF2,
    /// Command cannot be executed (dependency not fulfilled).
    ImpossibleCommand = 0xF3,
    /// Data will not fit into a frame supported by this server.
    ExceedsMaxApduLength = 0xF4,
    /// Attempt to write data beyond reserved resource.
    DataOverflow = 0xF5,
    /// Write value below minimum supported value.
    OutOfMinRange = 0xF6,
    /// Write value exceeds maximum supported value.
    OutOfMaxRange = 0xF7,
    /// Request contains invalid data.
    DataVoid = 0xF8,
    /// Data access not possible at this time.
    TemporarilyNotAvailable = 0xF9,
    /// Read access to write-only resource.
    AccessWriteOnly = 0xFA,
    /// Write access to read-only resource.
    AccessReadOnly = 0xFB,
    /// Access denied (authorization/security).
    AccessDenied = 0xFC,
    /// Resource not present, address does not exist.
    AddressVoid = 0xFD,
    /// Write access with wrong datatype (datapoint length).
    DataTypeConflict = 0xFE,
    /// Generic error.
    GenericError = 0xFF,
}

/// CEMI error codes for property access.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CemiErrorCode {
    /// Unknown error.
    Unspecified = 0x00,
    /// Write value not allowed.
    OutOfRange = 0x01,
    /// Write value too high.
    OutOfMaxRange = 0x02,
    /// Write value too low.
    OutOfMinRange = 0x03,
    /// Memory cannot be written or only with faults.
    MemoryError = 0x04,
    /// Write access to read-only property.
    ReadOnly = 0x05,
    /// Command not valid or not supported.
    IllegalCommand = 0x06,
    /// Access to non-existing property.
    VoidDatapoint = 0x07,
    /// Wrong data type (datapoint length).
    TypeConflict = 0x08,
    /// Non-existing property array index.
    PropertyIndexRangeError = 0x09,
    /// Property exists but cannot be written at this moment.
    ValueTemporarilyNotWriteable = 0x0A,
}
