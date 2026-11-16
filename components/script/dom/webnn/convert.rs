/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Conversions between the WebNN bindings and rustnn.

use std::ffi::CString;
use std::fmt::Display;

use rustnn::operator_enums::MLOperandDataType as RustnnDataType;

use crate::dom::bindings::codegen::Bindings::WebNNBinding::{
    MLOperandDataType, MLOperandDescriptor,
};
use crate::dom::bindings::error::{Error, Fallible};
use crate::dom::webnn::backend::BackendError;

pub(crate) fn to_rustnn_data_type(data_type: MLOperandDataType) -> RustnnDataType {
    match data_type {
        MLOperandDataType::Float32 => RustnnDataType::Float32,
        MLOperandDataType::Float16 => RustnnDataType::Float16,
        MLOperandDataType::Int32 => RustnnDataType::Int32,
        MLOperandDataType::Uint32 => RustnnDataType::Uint32,
        MLOperandDataType::Int64 => RustnnDataType::Int64,
        MLOperandDataType::Uint64 => RustnnDataType::Uint64,
        MLOperandDataType::Int8 => RustnnDataType::Int8,
        MLOperandDataType::Uint8 => RustnnDataType::Uint8,
    }
}

/// rustnn also knows int4 and uint4, which the bindings don't expose; operands of those types
/// can't be created through the API.
pub(crate) fn from_rustnn_data_type(data_type: RustnnDataType) -> MLOperandDataType {
    match data_type {
        RustnnDataType::Float32 => MLOperandDataType::Float32,
        RustnnDataType::Float16 => MLOperandDataType::Float16,
        RustnnDataType::Int32 => MLOperandDataType::Int32,
        RustnnDataType::Uint32 => MLOperandDataType::Uint32,
        RustnnDataType::Int64 => MLOperandDataType::Int64,
        RustnnDataType::Uint64 => MLOperandDataType::Uint64,
        RustnnDataType::Int8 | RustnnDataType::Int4 => MLOperandDataType::Int8,
        RustnnDataType::Uint8 | RustnnDataType::Uint4 => MLOperandDataType::Uint8,
    }
}

/// The size in bytes of one element of `data_type`.
pub(crate) fn element_size(data_type: MLOperandDataType) -> u64 {
    match data_type {
        MLOperandDataType::Float32 | MLOperandDataType::Int32 | MLOperandDataType::Uint32 => 4,
        MLOperandDataType::Float16 => 2,
        MLOperandDataType::Int64 | MLOperandDataType::Uint64 => 8,
        MLOperandDataType::Int8 | MLOperandDataType::Uint8 => 1,
    }
}

/// The largest tensor, in bytes, that the implementation accepts.
pub(crate) const MAX_TENSOR_BYTE_LENGTH: u64 = i32::MAX as u64;

/// <https://webmachinelearning.github.io/webnn/#valid-dimension>
/// and <https://webmachinelearning.github.io/webnn/#mloperanddescriptor-byte-length>
///
/// Checks that every dimension is valid and returns the descriptor's byte length.
pub(crate) fn checked_byte_length(descriptor: &MLOperandDescriptor) -> Fallible<u64> {
    let mut length = element_size(descriptor.dataType);
    for &dimension in descriptor.shape.iter() {
        if dimension == 0 {
            return Err(type_error("Tensor dimensions must be greater than zero"));
        }
        length = length
            .checked_mul(dimension as u64)
            .filter(|length| *length <= MAX_TENSOR_BYTE_LENGTH)
            .ok_or_else(|| type_error("The tensor is too large"))?;
    }
    Ok(length)
}

/// Validate `descriptor` and convert it to rustnn's.
pub(crate) fn to_rustnn_descriptor(
    descriptor: &MLOperandDescriptor,
) -> Fallible<rustnn::mlcontext::MLOperandDescriptor> {
    checked_byte_length(descriptor)?;
    Ok(rustnn::mlcontext::MLOperandDescriptor::new(
        to_rustnn_data_type(descriptor.dataType),
        descriptor
            .shape
            .iter()
            .map(|&dimension| dimension as u64)
            .collect(),
    ))
}

pub(crate) fn type_error(message: impl Display) -> Error {
    let message = message.to_string().replace('\0', "");
    Error::Type(CString::new(message).unwrap_or_default())
}

pub(crate) fn backend_error(error: BackendError) -> Error {
    match error {
        BackendError::NotSupported(message) => Error::NotSupported(Some(message)),
        BackendError::Operation(message) => Error::Operation(Some(message)),
    }
}
