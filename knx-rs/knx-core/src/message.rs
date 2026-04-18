// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Fabian Schmieder

//! CEMI message codes and APDU/TPDU service types.

/// CEMI message code — identifies the service type of a CEMI frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MessageCode {
    // L_Data services
    /// `L_Data.req` — data request from DLL.
    LDataReq = 0x11,
    /// `L_Data.con` — data confirmation.
    LDataCon = 0x2E,
    /// `L_Data.ind` — data indication (received frame).
    LDataInd = 0x29,

    // Management property services
    /// `M_PropRead.req`
    PropReadReq = 0xFC,
    /// `M_PropRead.con`
    PropReadCon = 0xFB,
    /// `M_PropWrite.req`
    PropWriteReq = 0xF6,
    /// `M_PropWrite.con`
    PropWriteCon = 0xF5,
    /// `M_PropInfo.ind`
    PropInfoInd = 0xF7,

    // Function property services
    /// `M_FuncPropCommand.req`
    FuncPropCommandReq = 0xF8,
    /// `M_FuncPropStateRead.req`
    FuncPropStateReadReq = 0xF9,
    /// `M_FuncPropCommand.con` / `M_FuncPropStateRead.con` (same code per spec).
    FuncPropCon = 0xFA,

    // Reset services
    /// `M_Reset.req`
    ResetReq = 0xF1,
    /// `M_Reset.ind`
    ResetInd = 0xF0,
}

/// Transport layer PDU type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TpduType {
    /// Broadcast data.
    DataBroadcast,
    /// Group-addressed data.
    DataGroup,
    /// Individually-addressed data (connectionless).
    DataIndividual,
    /// Connection-oriented data.
    DataConnected,
    /// Connection request.
    Connect,
    /// Disconnection request.
    Disconnect,
    /// Positive acknowledgement (connection-oriented).
    Ack,
    /// Negative acknowledgement (connection-oriented).
    Nack,
}

/// Application layer service type (APDU type).
///
/// Values match the wire encoding from the KNX specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum ApduType {
    // ── Multicast (group) services ────────────────────────────
    /// Read a group object value.
    GroupValueRead = 0x000,
    /// Response to a group value read.
    GroupValueResponse = 0x040,
    /// Write a group object value.
    GroupValueWrite = 0x080,

    // ── Broadcast services ────────────────────────────────────
    /// Write an individual address.
    IndividualAddressWrite = 0x0C0,
    /// Read individual addresses.
    IndividualAddressRead = 0x100,
    /// Response to individual address read.
    IndividualAddressResponse = 0x140,
    /// Read individual address by serial number.
    IndividualAddressSerialNumberRead = 0x3DC,
    /// Response to serial number read.
    IndividualAddressSerialNumberResponse = 0x3DD,
    /// Write individual address by serial number.
    IndividualAddressSerialNumberWrite = 0x3DE,

    // ── System broadcast services ─────────────────────────────
    /// System network parameter read.
    SystemNetworkParameterRead = 0x1C8,
    /// System network parameter response.
    SystemNetworkParameterResponse = 0x1C9,
    /// System network parameter write.
    SystemNetworkParameterWrite = 0x1CA,

    // ── Domain address services (RF) ──────────────────────────
    /// Domain address write.
    DomainAddressWrite = 0x3E0,
    /// Domain address read.
    DomainAddressRead = 0x3E1,
    /// Domain address response.
    DomainAddressResponse = 0x3E2,
    /// Domain address selective read.
    DomainAddressSelectiveRead = 0x3E3,
    /// Domain address serial number read.
    DomainAddressSerialNumberRead = 0x3EC,
    /// Domain address serial number response.
    DomainAddressSerialNumberResponse = 0x3ED,
    /// Domain address serial number write.
    DomainAddressSerialNumberWrite = 0x3EE,

    // ── Point-to-point services ───────────────────────────────
    /// ADC read.
    AdcRead = 0x180,
    /// ADC response.
    AdcResponse = 0x1C0,

    // ── Extended property services ────────────────────────────
    /// Property value extended read.
    PropertyValueExtRead = 0x1CC,
    /// Property value extended response.
    PropertyValueExtResponse = 0x1CD,
    /// Property value extended write (confirmed).
    PropertyValueExtWriteCon = 0x1CE,
    /// Property value extended write confirmed response.
    PropertyValueExtWriteConResponse = 0x1CF,
    /// Property value extended write (unconfirmed).
    PropertyValueExtWriteUnCon = 0x1D0,
    /// Property extended description read.
    PropertyExtDescriptionRead = 0x1D2,
    /// Property extended description response.
    PropertyExtDescriptionResponse = 0x1D3,
    /// Function property extended command.
    FunctionPropertyExtCommand = 0x1D4,
    /// Function property extended state.
    FunctionPropertyExtState = 0x1D5,
    /// Function property extended state response.
    FunctionPropertyExtStateResponse = 0x1D6,

    // ── Memory services ───────────────────────────────────────
    /// Extended memory write.
    MemoryExtWrite = 0x1FB,
    /// Extended memory write response.
    MemoryExtWriteResponse = 0x1FC,
    /// Extended memory read.
    MemoryExtRead = 0x1FD,
    /// Extended memory read response.
    MemoryExtReadResponse = 0x1FE,
    /// Memory read.
    MemoryRead = 0x200,
    /// Memory response.
    MemoryResponse = 0x240,
    /// Memory write.
    MemoryWrite = 0x280,
    /// User memory read.
    UserMemoryRead = 0x2C0,
    /// User memory response.
    UserMemoryResponse = 0x2C1,
    /// User memory write.
    UserMemoryWrite = 0x2C2,

    // ── Manufacturer info ─────────────────────────────────────
    /// User manufacturer info read.
    UserManufacturerInfoRead = 0x2C5,
    /// User manufacturer info response.
    UserManufacturerInfoResponse = 0x2C6,

    // ── Function property services ────────────────────────────
    /// Function property command.
    FunctionPropertyCommand = 0x2C7,
    /// Function property state read.
    FunctionPropertyState = 0x2C8,
    /// Function property state response.
    FunctionPropertyStateResponse = 0x2C9,

    // ── Device management ─────────────────────────────────────
    /// Device descriptor read.
    DeviceDescriptorRead = 0x300,
    /// Device descriptor response.
    DeviceDescriptorResponse = 0x340,
    /// Basic restart.
    Restart = 0x380,
    /// Master reset restart.
    RestartMasterReset = 0x381,

    // ── Routing table services ────────────────────────────────
    /// Routing table open.
    RoutingTableOpen = 0x3C0,
    /// Routing table read.
    RoutingTableRead = 0x3C1,
    /// Routing table read response.
    RoutingTableReadResponse = 0x3C2,
    /// Routing table write.
    RoutingTableWrite = 0x3C3,
    /// Memory router write.
    MemoryRouterWrite = 0x3CA,
    /// Memory router read response.
    MemoryRouterReadResponse = 0x3C9,

    // ── Authorization services ────────────────────────────────
    /// Authorize request.
    AuthorizeRequest = 0x3D1,
    /// Authorize response.
    AuthorizeResponse = 0x3D2,
    /// Key write.
    KeyWrite = 0x3D3,
    /// Key response.
    KeyResponse = 0x3D4,

    // ── Property services ─────────────────────────────────────
    /// Property value read.
    PropertyValueRead = 0x3D5,
    /// Property value response.
    PropertyValueResponse = 0x3D6,
    /// Property value write.
    PropertyValueWrite = 0x3D7,
    /// Property description read.
    PropertyDescriptionRead = 0x3D8,
    /// Property description response.
    PropertyDescriptionResponse = 0x3D9,

    // ── Secure service ────────────────────────────────────────
    /// KNX Data Secure service wrapper.
    SecureService = 0x3F1,
}
