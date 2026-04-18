// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Fabian Schmieder

//! `knx-device` — KNX device stack with ETS programming support.
//!
//! Provides group objects, interface objects, address/association tables,
//! transport/application layer state machines, and memory management.

#![no_std]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

extern crate alloc;
