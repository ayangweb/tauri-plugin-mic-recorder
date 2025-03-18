use chrono::Local;
use clap::Parser;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    FromSample, Sample, Stream,
};
use hound::{SampleFormat, WavSpec, WavWriter};
use std::{
    fs::{create_dir_all, File},
    io::BufWriter,
    marker::{Send, Sync},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, LazyLock, Mutex,
    },
};
use tauri::{command, AppHandle, Manager, Runtime};

type WavWriterHandle = Arc<Mutex<Option<WavWriter<BufWriter<File>>>>>;

struct SafeStream(Stream);

unsafe impl Send for SafeStream {}
unsafe impl Sync for SafeStream {}

struct State {
    is_recording: Arc<AtomicBool>,
    save_path: Arc<Mutex<Option<PathBuf>>>,
    writer: WavWriterHandle,
    stream: Arc<Mutex<Option<SafeStream>>>,
}

impl State {
    fn new() -> Self {
        Self {
            is_recording: Arc::new(AtomicBool::new(false)),
            save_path: Arc::new(Mutex::new(None)),
            writer: Arc::new(Mutex::new(None)),
            stream: Arc::new(Mutex::new(None)),
        }
    }
}

static STATE: LazyLock<Arc<Mutex<State>>> = LazyLock::new(|| Arc::new(Mutex::new(State::new())));

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
    let mut state = STATE.lock().map_err(|err| err.to_string())?;
    if state.is_recording.load(Ordering::SeqCst) {
        return Err("Recording is already in progress.".to_string());
    }
    state.is_recording.store(true, Ordering::SeqCst);

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
        cpal::host_from_id(
            cpal::available_hosts()
                .into_iter()
                .find(|id| *id == cpal::HostId::Jack)
                .ok_or("JACK host not available. Make sure --features jack is specified.")?,
        )
        .map_err(|err| err.to_string())?
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
            .ok_or("No default input device available")?
    } else {
        host.input_devices()
            .map_err(|err| err.to_string())?
            .find(|x| x.name().map(|y| y == opt.device).unwrap_or(false))
            .ok_or(format!("No input device found with name: {}", opt.device))?
    };

    let config = device
        .default_input_config()
        .map_err(|err| err.to_string())?;

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
        _ => return Err("Unsupported sample format".to_string()),
    };

    stream.play().map_err(|err| err.to_string())?;

    *state.save_path.lock().map_err(|err| err.to_string())? = Some(save_path);
    state.writer = writer;
    *state.stream.lock().map_err(|err| err.to_string())? = Some(SafeStream(stream));

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
    let state = STATE.lock().map_err(|err| err.to_string())?;
    if !state.is_recording.load(Ordering::SeqCst) {
        return Err("No recording in progress.".to_string());
    }
    state.is_recording.store(false, Ordering::SeqCst);

    // Stop the stream
    if let Some(stream) = state.stream.lock().map_err(|err| err.to_string())?.take() {
        drop(stream.0);
    }

    // Finalize the writer
    if let Some(writer) = state.writer.lock().map_err(|err| err.to_string())?.take() {
        writer.finalize().map_err(|err| err.to_string())?;
    }

    // Get and clear the save path
    let save_path = state
        .save_path
        .lock()
        .map_err(|err| err.to_string())?
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

    Ok(save_path)
}

/// Converts a cpal::SampleFormat to a hound::SampleFormat.
fn sample_format(format: cpal::SampleFormat) -> SampleFormat {
    if format.is_float() {
        SampleFormat::Float
    } else {
        SampleFormat::Int
    }
}

/// Creates a WavSpec from a cpal::SupportedStreamConfig.
fn wav_spec_from_config(config: &cpal::SupportedStreamConfig) -> WavSpec {
    WavSpec {
        channels: config.channels() as _,
        sample_rate: config.sample_rate().0 as _,
        bits_per_sample: (config.sample_format().sample_size() * 8) as _,
        sample_format: sample_format(config.sample_format()),
    }
}

/// Writes input data to the WAV writer.
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
