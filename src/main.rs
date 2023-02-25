//! Feeds back the input stream directly into the output stream.
//!
//! Assumes that the input and output devices can use the same stream configuration and that they
//! support the f32 sample format.
//!
//! Uses a delay of `LATENCY_MS` milliseconds in case the default input and output streams are not
//! precisely synchronised.

use clap::Parser;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use anyhow;
use hound::WavReader;

#[derive(Parser, Debug)]
#[command(version, about = "CPAL feedback example", long_about = None)]
struct Opt {
    #[arg(short = 'v', long, value_name = "LIST_DEVICES", default_value_t = false)]
    list_devices: bool,

    #[arg(short, long, value_name = "OUT", default_value_t = String::from("default"))]
    output_device: String,

    #[arg(short, long, value_name = "WAV_FILE")]
    wav_file: String,

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

fn setup_host(_opt: &Opt) -> cpal::Host {
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
    if _opt.jack {
        cpal::host_from_id(cpal::available_hosts()
            .into_iter()
            .find(|id| *id == cpal::HostId::Jack)
            .expect(
                "make sure --features jack is specified. only works on OSes where jack is available",
            )).expect("jack host unavailable")
    } else {
        cpal::default_host()
    }

    #[cfg(any(
        not(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd"
        )),
        not(feature = "jack")
    ))]
    cpal::default_host()
}

fn main() -> anyhow::Result<()> {
    let opt = Opt::parse();
    
    let host = setup_host(&opt);

    if opt.list_devices {
        println!("output_device devices");
        for output_device in host.output_devices()? {
            println!("{}", output_device.name()?);
        }
        return Ok(())
    }

    let output_device = if opt.output_device == "default" {
        host.default_output_device()
    } else {
        host.output_devices()?
            .find(|x| x.name().map(|y| y == opt.output_device).unwrap_or(false))
    }
    .expect("failed to find output device");

    println!("Using output device: \"{}\"", output_device.name()?);

    // We'll try and use the same configuration between streams to keep it simple.
    let config: cpal::StreamConfig = output_device.default_output_config()?.into();

    let mut reader = WavReader::open(opt.wav_file).unwrap();
    let wav_length = reader.len() as usize;
    let mut count = 0 as usize;

    let output_data_fn = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
        // alternative to cycle
        //let (front, back) = v.split_at(3);
        //for element in back.iter().chain(front) { /* loop */ }
        //cycle().skip(count).take(config.buffer_size) 
        let reader_iter = reader.samples::<i16>();
        for (sample, wav_sample) in data.iter_mut().zip(reader_iter) {
            let wav_sample = wav_sample.unwrap() as f32;
            // convert wav_sample to f3
            *sample = wav_sample;
        }
        
        let buffer_size = data.len() as usize;
        count = (count + buffer_size) % wav_length;
    };

    // Build streams.
    println!(
        "Attempting to build both streams with f32 samples and `{:?}`.",
        config
    );
    let output_stream = output_device.build_output_stream(&config, output_data_fn, err_fn, None)?;
    println!("Successfully built streams.");

    output_stream.play()?;

    // Run for 3 seconds before closing.
    println!("Playing for 3 seconds... ");
    std::thread::sleep(std::time::Duration::from_secs(3));
    drop(output_stream);
    println!("Done!");
    Ok(())
}

fn err_fn(err: cpal::StreamError) {
    eprintln!("an error occurred on stream: {}", err);
}
