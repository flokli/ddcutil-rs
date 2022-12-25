use std::{fmt, mem};
use std::collections::HashMap;
use std::ffi::CStr;
use std::slice::from_raw_parts;

use bitflags::bitflags;
use libc::c_char;

use crate::{Error, Result, sys};

pub type FeatureCode = u8;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Value {
    pub mh: u8,
    pub ml: u8,
    pub sh: u8,
    pub sl: u8,
}

impl Value {
    pub fn from_raw(raw: &sys::DDCA_Non_Table_Vcp_Value) -> Self {
        Value {
            mh: raw.mh,
            ml: raw.ml,
            sh: raw.sh,
            sl: raw.sl,
        }
    }

    pub fn value(&self) -> u16 {
        ((self.sh as u16) << 8) | self.sl as u16
    }

    pub fn maximum(&self) -> u16 {
        ((self.mh as u16) << 8) | self.ml as u16
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MccsVersion {
    pub major: u8,
    pub minor: u8,
}

impl MccsVersion {
    pub fn from_raw(raw: sys::DDCA_MCCS_Version_Spec) -> Self {
        MccsVersion {
            major: raw.major,
            minor: raw.minor,
        }
    }

    pub fn to_raw(&self) -> sys::DDCA_MCCS_Version_Spec {
        sys::DDCA_MCCS_Version_Spec {
            major: self.major,
            minor: self.minor,
        }
    }
}

impl fmt::Display for MccsVersion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

impl fmt::Debug for MccsVersion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capabilities {
    pub version: MccsVersion,
    pub features: HashMap<FeatureCode, Vec<u8>>,
}

impl Capabilities {
    pub unsafe fn from_raw(raw: &sys::DDCA_Capabilities) -> Self {
        let version = MccsVersion::from_raw(raw.version_spec);
        let features= from_raw_parts(raw.vcp_codes, raw.vcp_code_ct as usize)
            .iter()
            .map(|feature| (
                feature.feature_code,
                from_raw_parts(feature.values, feature.value_ct as usize).to_owned()
            ))
            .collect();

        Capabilities { version, features }
    }

    pub fn from_cstr(caps: &CStr) -> Result<Self> {
        unsafe {
            let mut raw_caps = mem::MaybeUninit::uninit();
            Error::from_status(sys::ddca_parse_capabilities_string(
                caps.as_ptr() as *mut _, raw_caps.as_mut_ptr()
            ))?;
            let raw_caps = raw_caps.assume_init();
            let caps = Capabilities::from_raw(&*raw_caps);
            sys::ddca_free_parsed_capabilities(raw_caps);
            Ok(caps)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureMetadata {
    pub name: String,
    pub description: String,
    pub value_names: HashMap<u8, String>,
    pub flags: FeatureFlags,
}

impl FeatureMetadata {
    pub fn from_code(code: FeatureCode, version: MccsVersion) -> Result<Self> {
        unsafe {
            let mut meta = mem::MaybeUninit::uninit();
            Error::from_status(sys::ddca_get_feature_metadata_by_vspec(
                code, version.to_raw(), false, meta.as_mut_ptr(),
            ))?;
            let meta = meta.assume_init();
            let features = Self::from_raw(&*meta);
            sys::ddca_free_feature_metadata(meta);
            Ok(features)
        }
    }

    pub unsafe fn from_raw(raw: &sys::DDCA_Feature_Metadata) -> Self {
        unsafe fn from_ptr(ptr: *const c_char) -> String {
            if ptr.is_null() {
                Default::default()
            } else {
                CStr::from_ptr(ptr).to_string_lossy().into_owned()
            }
        }

        FeatureMetadata {
            name: from_ptr(raw.feature_name),
            description: from_ptr(raw.feature_desc),
            value_names: Self::sl_values(raw).iter().map(|v| (
                v.value_code,
                from_ptr(v.value_name),
            )).collect(),
            flags: FeatureFlags::from_bits_truncate(raw.feature_flags),
        }
    }

    fn sl_values_len(meta: &sys::DDCA_Feature_Metadata) -> usize {
        if meta.feature_flags & sys::DDCA_SIMPLE_NC as u16 != 0 {
            let mut ptr = meta.sl_values;
            let mut len = 0;
            unsafe {
                while (*ptr).value_code != 0 || !(*ptr).value_name.is_null() {
                    ptr = ptr.offset(1);
                    len += 1;
                }
            }

            len
        } else {
            0
        }
    }

    fn sl_values(meta: &sys::DDCA_Feature_Metadata) -> &[sys::DDCA_Feature_Value_Entry] {
        let len = Self::sl_values_len(meta);
        if len == 0 {
            &[]
        } else {
            unsafe {
                from_raw_parts(meta.sl_values as *const _, len)
            }
        }
    }
}

bitflags! {
    // FIXME: How to make bindgen create the correct int type for constants?
    pub struct FeatureFlags: u16 {
        /// Read only feature
        const RO = sys::DDCA_RO as u16;
        /// Write only feature
        const WO = sys::DDCA_WO as u16;
        /// Feature is both readable and writable
        const RW = sys::DDCA_RW as u16;

        /// Normal continuous feature
        const STD_CONT = sys::DDCA_STD_CONT as u16;
        /// Continuous feature with special interpretation
        const COMPLEX_CONT = sys::DDCA_COMPLEX_CONT as u16;
        /// Non-continuous feature, having a defined list of values in byte SL
        const SIMPLE_NC = sys::DDCA_SIMPLE_NC as u16;
        /// Non-continuous feature, having a complex interpretation using one or more of SL, SH, ML, MH
        const COMPLEX_NC = sys::DDCA_COMPLEX_NC as u16;

        /// Used internally for write-only non-continuous features
        const WO_NC = sys::DDCA_WO_NC as u16;
        /// Normal RW table type feature
        const NORMAL_TABLE = sys::DDCA_NORMAL_TABLE as u16;
        /// Write only table feature
        const WO_TABLE = sys::DDCA_WO_TABLE as u16;

        /// Feature is deprecated in the specified VCP version
        const DEPRECATED = sys::DDCA_DEPRECATED as u16;

        /// DDCA_Global_Feature_Flags
        const SYNTHETIC = sys::DDCA_SYNTHETIC as u16;
    }
}

impl FeatureFlags {
    /// Feature is either RW or RO
    pub fn is_readable(&self) -> bool {
        self.bits & sys::DDCA_READABLE as u16 != 0
    }

    /// Feature is either RW or WO
    pub fn is_writable(&self) -> bool {
        self.bits & sys::DDCA_WRITABLE as u16 != 0
    }

    /// Continuous feature, of any subtype
    pub fn is_cont(&self) -> bool {
        self.bits & sys::DDCA_CONT as u16 != 0
    }

    /// Non-continuous feature of any subtype
    pub fn is_nc(&self) -> bool {
        self.bits & sys::DDCA_NC as u16 != 0
    }

    /// Non-table feature of any type
    pub fn is_non_table(&self) -> bool {
        self.bits & sys::DDCA_NON_TABLE as u16 != 0
    }

    /// Table type feature, of any subtype
    pub fn is_table(&self) -> bool {
        self.bits & sys::DDCA_TABLE as u16 != 0
    }

    /// unused
    pub fn is_known(&self) -> bool {
        self.is_nc() || self.is_cont() || self.is_table()
    }
}
