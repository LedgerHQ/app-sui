use ledger_device_sdk::nvm::*;
use ledger_device_sdk::NVMData;

// This is necessary to store the object in NVM and not in RAM
const SETTINGS_SIZE: usize = 10;
#[link_section = ".nvm_data"]
static mut SETTINGS: NVMData<AtomicStorage<[u8; SETTINGS_SIZE]>> =
    NVMData::new(AtomicStorage::new(&[0u8; 10]));

const BLINDSIGN_IX: usize = 0;

#[derive(Clone, Copy)]
pub struct Settings;

impl Default for Settings {
    fn default() -> Self {
        Settings
    }
}

impl Settings {
    #[inline(never)]
    pub fn get_mut(&mut self) -> &mut AtomicStorage<[u8; SETTINGS_SIZE]> {
        #[allow(static_mut_refs)]
        unsafe {
            SETTINGS.get_mut()
        }
    }

    #[inline(never)]
    pub fn get_blind_sign(&self) -> bool {
        #[allow(static_mut_refs)]
        let settings = unsafe { SETTINGS.get_ref() };
        settings.get_ref()[BLINDSIGN_IX] == 1
    }

    // The inline(never) is important. Otherwise weird segmentation faults happen on speculos.
    #[inline(never)]
    pub fn set_blind_sign(&mut self, enabled: bool) {
        #[allow(static_mut_refs)]
        let settings = unsafe { SETTINGS.get_mut() };
        let mut switch_values: [u8; SETTINGS_SIZE] = *settings.get_ref();
        if enabled {
            switch_values[BLINDSIGN_IX] = 1;
        } else {
            switch_values[BLINDSIGN_IX] = 0;
        }
        settings.update(&switch_values);
    }
}
