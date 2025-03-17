use chrono::Local;
use clap::Parser;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    FromSample, Sample,
};
use hound::{SampleFormat, WavSpec, WavWriter};
use std::{
    fs::{create_dir_all, File},
    io::BufWriter,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use std::{sync::LazyLock, thread::sleep};
use tauri::{command, AppHandle, Manager, Runtime};

type WavWriterHandle = Arc<Mutex<Option<WavWriter<BufWriter<File>>>>>;

static IS_RECORDING: AtomicBool = AtomicBool::new(false);
static SAVE_PATH: LazyLock<Arc<Mutex<Option<PathBuf>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(None)));

#[derive(Parser, Debug)]
struct Opt {
    /// The audio device to use
    #[arg(short, long, default_value_t = String::from("default"))]
    device: String,

    /// Use the JACK host
    #[cfg(all(
        any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd"
        ),
        feature = "jack"
    ))]
    #[arg(short, long)]
    #[allow(dead_code)]
    jack: bool,
}

/// Starts recording audio.
///
/// # Examples
/// ```
/// use tauri_plugin_mic_recorder::start_recording;
///
/// start_recording().unwrap();
/// ```
#[command]
pub async fn start_recording<R: Runtime>(app_handle: AppHandle<R>) -> Result<(), String> {
    if IS_RECORDING.load(Ordering::SeqCst) {
        return Err("Recording is already in progress.".to_string());
    }

    IS_RECORDING.store(true, Ordering::SeqCst);

    let opt = Opt::parse();

    // Conditionally compile with jack if the feature is specified.
    #[cfg(all(
        any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd"
        ),
        feature = "jack"
    ))]
    // Manually check for flags. Can be passed through cargo with -- e.g.
    // cargo run --release --example beep --features jack -- --jack
    let host = if opt.jack {
        cpal::host_from_id(cpal::available_hosts()
            .into_iter()
            .find(|id| *id == cpal::HostId::Jack)
            .expect(
                "make sure --features jack is specified. only works on OSes where jack is available",
            )).expect("jack host unavailable")
    } else {
        cpal::default_host()
    };

    #[cfg(any(
        not(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd"
        )),
        not(feature = "jack")
    ))]
    let host = cpal::default_host();

    // Set up the input device and stream with the default input config.
    let device = if opt.device == "default" {
        host.default_input_device()
    } else {
        host.input_devices()
            .map_err(|err| err.to_string())?
            .find(|x| x.name().map(|y| y == opt.device).unwrap_or(false))
    }
    .expect("failed to find input device");

    let config = device
        .default_input_config()
        .expect("Failed to get default input config");

    let save_path = get_save_path(&app_handle)?;
    // The WAV file we're recording to.
    let spec = wav_spec_from_config(&config);
    let writer = WavWriter::create(&save_path, spec).map_err(|err| err.to_string())?;
    let writer = Arc::new(Mutex::new(Some(writer)));

    // Run the input stream on a separate thread.
    let writer_2 = writer.clone();

    let err_fn = move |err: cpal::StreamError| {
        eprintln!("an error occurred on stream: {}", err);
    };

    let stream = match config.sample_format() {
        cpal::SampleFormat::I8 => device
            .build_input_stream(
                &config.into(),
                move |data, _: &_| write_input_data::<i8, i8>(data, &writer_2),
                err_fn,
                None,
            )
            .map_err(|err| err.to_string())?,
        cpal::SampleFormat::I16 => device
            .build_input_stream(
                &config.into(),
                move |data, _: &_| write_input_data::<i16, i16>(data, &writer_2),
                err_fn,
                None,
            )
            .map_err(|err| err.to_string())?,
        cpal::SampleFormat::I32 => device
            .build_input_stream(
                &config.into(),
                move |data, _: &_| write_input_data::<i32, i32>(data, &writer_2),
                err_fn,
                None,
            )
            .map_err(|err| err.to_string())?,
        cpal::SampleFormat::F32 => device
            .build_input_stream(
                &config.into(),
                move |data, _: &_| write_input_data::<f32, f32>(data, &writer_2),
                err_fn,
                None,
            )
            .map_err(|err| err.to_string())?,
        sample_format => return Err("Unsupported sample format '{sample_format}'".to_string()),
    };

    stream.play().map_err(|err| err.to_string())?;

    while IS_RECORDING.load(Ordering::SeqCst) {
        sleep(Duration::from_millis(100));
    }

    drop(stream);

    writer
        .lock()
        .map_err(|err| err.to_string())?
        .take()
        .ok_or("Wav writer is unexpectedly missing".to_string())?
        .finalize()
        .map_err(|err| err.to_string())?;

    Ok(())
}

/// Stops recording audio.
///
/// # Returns
/// - `Ok(PathBuf)`: Returns the path where the recording file is stored.
/// - `Err(String)`: An error message string on failure.
///
/// # Examples
/// ```
/// use tauri_plugin_mic_recorder::stop_recording;
///
/// let save_path = stop_recording().unwrap();
/// println!("Recording saved to: {:?}", save_path);
/// ```
#[command]
pub async fn stop_recording() -> Result<PathBuf, String> {
    IS_RECORDING.store(false, Ordering::SeqCst);

    let mut save_path_guard = SAVE_PATH.lock().map_err(|err| err.to_string())?;
    let save_path = save_path_guard
        .take()
        .ok_or("No recording in progress or save path not set.".to_string())?;

    Ok(save_path)
}

/// Gets the path where the recording file is stored.
fn get_save_path<R: Runtime>(app_handle: &AppHandle<R>) -> Result<PathBuf, String> {
    let save_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|err| err.to_string())?
        .join("tauri-plugin-mic-recorder");

    create_dir_all(&save_dir).map_err(|err| err.to_string())?;

    let timestamp = Local::now().format("%Y%m%d%H%M%S").to_string();
    let save_path = save_dir.join(format!("{timestamp}.wav"));

    let mut save_path_guard = SAVE_PATH.lock().map_err(|err| err.to_string())?;
    *save_path_guard = Some(save_path.clone());

    Ok(save_path)
}

fn sample_format(format: cpal::SampleFormat) -> SampleFormat {
    if format.is_float() {
        SampleFormat::Float
    } else {
        SampleFormat::Int
    }
}

fn wav_spec_from_config(config: &cpal::SupportedStreamConfig) -> WavSpec {
    WavSpec {
        channels: config.channels() as _,
        sample_rate: config.sample_rate().0 as _,
        bits_per_sample: (config.sample_format().sample_size() * 8) as _,
        sample_format: sample_format(config.sample_format()),
    }
}

fn write_input_data<T, U>(input: &[T], writer: &WavWriterHandle)
where
    T: Sample,
    U: Sample + hound::Sample + FromSample<T>,
{
    if let Ok(mut guard) = writer.try_lock() {
        if let Some(writer) = guard.as_mut() {
            for &sample in input.iter() {
                let sample: U = U::from_sample(sample);
                writer.write_sample(sample).ok();
            }
        }
    }
}
