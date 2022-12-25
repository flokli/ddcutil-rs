use std::{mem, fmt};
use std::borrow::Cow;
use std::ffi::{CStr, CString};
use std::slice::from_raw_parts;
use libc::{c_int, c_char};
use crate::{sys, Error, Result, FeatureCode, Capabilities, Value};

#[derive(Clone)]
pub struct DisplayInfo {
    handle: sys::DDCA_Display_Ref,
    display_number: i32,
    manufacturer_id: Vec<u8>,
    model_name: Vec<u8>,
    serial_number: Vec<u8>,
    edid: Box<[u8]>,
    path: DisplayPath,
}

impl DisplayInfo {
    pub fn open(&self, wait: bool) -> Result<Display> {
        unsafe {
            let mut handle = mem::MaybeUninit::uninit();
            let status = sys::ddca_open_display2(self.handle, wait, handle.as_mut_ptr());
            Error::from_status(status).map(|_| Display::from_raw(handle.assume_init()))
        }
    }

    pub unsafe fn from_raw(raw: &sys::DDCA_Display_Info) -> Self {
        fn from_ptr(ptr: *const c_char) -> Vec<u8> {
            if ptr.is_null() {
                Default::default()
            } else {
                unsafe {
                    CStr::from_ptr(ptr).to_bytes().to_owned()
                }
            }
        }

        DisplayInfo {
            handle: raw.dref,
            display_number: raw.dispno,
            manufacturer_id: from_ptr(raw.mfg_id.as_ptr()),
            model_name: from_ptr(raw.model_name.as_ptr()),
            serial_number: from_ptr(raw.sn.as_ptr()),
            edid: raw.edid_bytes.to_owned().into(),
            path: DisplayPath::from_raw(&raw.path, raw.usb_bus, raw.usb_device)
                .unwrap_or_else(|_| DisplayPath::Usb {
                    // stupid fallback, but should never happen...
                    bus_number: raw.usb_bus,
                    device_number: raw.usb_device,
                    hiddev_device_number: -1,
                }),
        }
    }

    pub fn enumerate(include_invalid_display: bool) -> Result<DisplayInfoList<'static>> {
        unsafe {
            let mut raw = mem::MaybeUninit::uninit();
            let status = sys::ddca_get_display_info_list2(
                include_invalid_display,
                raw.as_mut_ptr(),
            );
            Error::from_status(status).map(|_| DisplayInfoList::from_raw(raw.assume_init()))
        }
    }

    pub fn raw(&self) -> sys::DDCA_Display_Ref {
        self.handle
    }

    pub fn display_number(&self) -> i32 {
        self.display_number
    }

    pub fn manufacturer_id(&self) -> Cow<str> {
        String::from_utf8_lossy(&self.manufacturer_id)
    }

    pub fn manufacturer_id_bytes(&self) -> &[u8] {
        &self.manufacturer_id
    }

    pub fn model_name(&self) -> Cow<str> {
        String::from_utf8_lossy(&self.model_name)
    }

    pub fn model_name_bytes(&self) -> &[u8] {
        &self.model_name
    }

    pub fn serial_number(&self) -> Cow<str> {
        String::from_utf8_lossy(&self.serial_number)
    }

    pub fn serial_number_bytes(&self) -> &[u8] {
        &self.serial_number
    }

    pub fn edid(&self) -> &[u8] {
        &self.edid
    }

    pub fn path(&self) -> DisplayPath {
        self.path
    }
}

impl fmt::Debug for DisplayInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("DisplayInfo")
            .field("display_number", &self.display_number)
            .field("manufacturer_id", &self.manufacturer_id())
            .field("model_name", &self.model_name())
            .field("serial_number", &self.serial_number())
            .field("path", &self.path())
            .finish()
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DisplayPath {
    I2c {
        bus_number: i32,
    },
    Usb {
        bus_number: i32,
        device_number: i32,
        hiddev_device_number: i32,
    },
    Adl {
        adapter_index: i32,
        display_index: i32,
    },
}

impl DisplayPath {
    pub unsafe fn from_raw(
        path: &sys::DDCA_IO_Path,
        usb_bus: c_int,
        usb_device: c_int,
    ) -> std::result::Result<Self, ()> {
        match path.io_mode {
            sys::DDCA_IO_Mode_DDCA_IO_I2C => Ok(DisplayPath::I2c {
                bus_number: path.path.i2c_busno,
            }),
            sys::DDCA_IO_Mode_DDCA_IO_USB => Ok(DisplayPath::Usb {
                bus_number: usb_bus as _,
                device_number: usb_device as _,
                hiddev_device_number: path.path.hiddev_devno,
            }),
            sys::DDCA_IO_Mode_DDCA_IO_ADL => Ok(DisplayPath::Adl {
                adapter_index: path.path.adlno.iAdapterIndex,
                display_index: path.path.adlno.iDisplayIndex,
            }),
            _ => Err(()),
        }
    }
}

pub struct DisplayInfoList<'a> {
    handle: *mut sys::DDCA_Display_Info_List,
    list: &'a [sys::DDCA_Display_Info]
}

unsafe impl Send for DisplayInfoList<'_> { }
unsafe impl Sync for DisplayInfoList<'_> { }

impl fmt::Debug for DisplayInfoList<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_list().entries(self.into_iter()).finish()
    }
}

impl DisplayInfoList<'_> {
    pub unsafe fn from_raw(handle: *mut sys::DDCA_Display_Info_List) -> Self {
        DisplayInfoList {
            handle,
            list: from_raw_parts((*handle).info.as_ptr(), (*handle).ct as usize)
        }
    }

    pub fn raw(&self) -> &sys::DDCA_Display_Info_List {
        unsafe { &*self.handle }
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn get(&self, index: usize) -> DisplayInfo {
        unsafe { DisplayInfo::from_raw(&self.list[index]) }
    }
}

impl<'a> IntoIterator for &'a DisplayInfoList<'a> {
    type Item = DisplayInfo;
    type IntoIter = DisplayInfoIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Self::IntoIter {
            list: self,
            index: 0,
        }
    }
}

impl Drop for DisplayInfoList<'_> {
    fn drop(&mut self) {
        unsafe { sys::ddca_free_display_info_list(self.handle) }
    }
}

#[derive(Copy, Clone)]
pub struct DisplayInfoIter<'a> {
    list: &'a DisplayInfoList<'a>,
    index: usize,
}

impl<'a> Iterator for DisplayInfoIter<'a> {
    type Item = DisplayInfo;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.list.len() {
            let index = self.index;
            self.index += 1;
            Some(self.list.get(index))
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct Display {
    handle: sys::DDCA_Display_Handle,
}
unsafe impl Send for Display { }

impl Display {
    pub unsafe fn from_raw(handle: sys::DDCA_Display_Handle) -> Self {
        Display {
            handle,
        }
    }

    pub fn capabilities_string(&self) -> Result<CString> {
        unsafe {
            let mut raw = mem::MaybeUninit::uninit();
            Error::from_status(sys::ddca_get_capabilities_string(
                self.handle, raw.as_mut_ptr()
            ))?;
            let raw = raw.assume_init();
            let string = CStr::from_ptr(raw).to_owned();
            libc::free(raw as *mut _);
            Ok(string)
        }
    }

    pub fn capabilities(&self) -> Result<Capabilities> {
        self.capabilities_string().and_then(|c| Capabilities::from_cstr(&c))
    }

    pub fn vcp_set_value(&self, code: FeatureCode, value: u8) -> Result<()> {
        unsafe {
            Error::from_status(sys::ddca_set_non_table_vcp_value(
                self.handle, code, value, 0
            )).map(|_| ())
        }
    }

    pub fn vcp_get_value(&self, code: FeatureCode) -> Result<Value> {
        unsafe {
            let mut raw = mem::MaybeUninit::uninit();
            Error::from_status(sys::ddca_get_non_table_vcp_value(
                self.handle, code, raw.as_mut_ptr()
            ))?;
            let value = Value::from_raw(&raw.assume_init());
            Ok(value)
        }
    }

    pub fn vcp_get_table(&self, code: FeatureCode) -> Result<Vec<u8>> {
        unsafe {
            let mut raw = mem::MaybeUninit::uninit();
            Error::from_status(sys::ddca_get_table_vcp_value(
                self.handle, code, raw.as_mut_ptr()
            ))?;
            let raw = raw.assume_init();
            let value = {
                let raw = &*raw;
                from_raw_parts(raw.bytes, raw.bytect as usize).to_owned()
            };
            sys::ddca_free_table_vcp_value(raw);
            Ok(value)
        }
    }

    pub fn raw(&self) -> sys::DDCA_Display_Handle {
        self.handle
    }
}

impl Drop for Display {
    fn drop(&mut self) {
        unsafe {
            sys::ddca_close_display(self.handle);
        }
    }
}

#[test]
fn test_displays() {
    for display in &DisplayInfo::enumerate(false).unwrap() {
        drop(display.open(true));
    }
}
