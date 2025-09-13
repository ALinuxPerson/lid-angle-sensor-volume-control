use lid_angle_sensor::LidAngleSensor;
use objc2_audio_toolbox::kAudioHardwareServiceDeviceProperty_VirtualMainVolume;
use objc2_core_audio::{
    AudioDeviceID, AudioObjectGetPropertyData, AudioObjectID, AudioObjectPropertyAddress,
    AudioObjectSetPropertyData, kAudioDevicePropertyScopeOutput,
    kAudioHardwarePropertyDefaultOutputDevice, kAudioObjectPropertyElementMain,
    kAudioObjectPropertyScopeGlobal, kAudioObjectSystemObject,
};
use objc2_core_foundation::kCFRunLoopCommonModes;
use std::mem::MaybeUninit;
use std::ptr::NonNull;
use std::time::Duration;
use std::{ptr, thread};

unsafe fn audio_object_get_property_data<T>(
    object_id: AudioObjectID,
    in_address: &AudioObjectPropertyAddress,
) -> Result<(T, usize), i32> {
    let mut data = MaybeUninit::<T>::uninit();
    let mut size = size_of::<T>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            object_id,
            NonNull::from_ref(in_address),
            0,
            ptr::null(),
            NonNull::from_ref(&mut size),
            NonNull::new_unchecked(data.as_mut_ptr() as *mut _),
        )
    };

    if status != 0 {
        return Err(status);
    }

    unsafe { Ok((data.assume_init(), size as usize)) }
}

unsafe fn audio_object_set_property_data<T>(
    object_id: AudioObjectID,
    in_address: &AudioObjectPropertyAddress,
    mut data: T,
) -> Result<(), i32> {
    let status = unsafe {
        AudioObjectSetPropertyData(
            object_id,
            NonNull::from_ref(in_address),
            0,
            ptr::null(),
            size_of::<T>() as _,
            NonNull::new_unchecked(&mut data as *mut T as *mut _),
        )
    };

    if status != 0 {
        return Err(status);
    }

    Ok(())
}

pub struct AudioDevice {
    id: AudioDeviceID,
}

impl AudioDevice {
    const VOLUME: AudioObjectPropertyAddress = AudioObjectPropertyAddress {
        mSelector: kAudioHardwareServiceDeviceProperty_VirtualMainVolume,
        mScope: kAudioDevicePropertyScopeOutput,
        mElement: kAudioObjectPropertyElementMain,
    };

    pub fn get() -> anyhow::Result<Self> {
        let in_address = AudioObjectPropertyAddress {
            mSelector: kAudioHardwarePropertyDefaultOutputDevice,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain,
        };
        let (device_id, _) = unsafe {
            audio_object_get_property_data(kAudioObjectSystemObject as _, &in_address).unwrap()
        };
        Ok(Self { id: device_id })
    }

    pub fn volume(&self) -> anyhow::Result<f32> {
        let (volume, _) =
            unsafe { audio_object_get_property_data(self.id, &Self::VOLUME).unwrap() };
        Ok(volume)
    }

    pub fn set_volume(&self, value: f32) -> anyhow::Result<()> {
        unsafe {
            audio_object_set_property_data(self.id, &Self::VOLUME, value.clamp(0.0, 1.0)).unwrap()
        };
        Ok(())
    }
}

fn quantized_clamp(value: u16) -> f32 {
    ((value as f32 / 5.0).round() * 5.0).clamp(0.0, 135.0) / 135.0
}

fn main() {
    let audio_device = AudioDevice::get().unwrap();
    let sensor = LidAngleSensor::new().unwrap().unwrap();

    let mut last_volume = quantized_clamp(sensor.lid_angle().unwrap());
    audio_device.set_volume(last_volume).unwrap();

    loop {
        let new_lid_angle = sensor.lid_angle().unwrap();
        let volume = quantized_clamp(new_lid_angle);
        println!(
            "New lid angle: {new_lid_angle}, new volume: {:.2}%",
            volume * 100.0,
        );

        if last_volume != volume {
            audio_device.set_volume(volume).unwrap();
        }

        last_volume = volume;
        thread::sleep(Duration::from_millis(100))
    }
}
