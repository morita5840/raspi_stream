use std::process;

#[derive(Debug, Clone)]
pub(crate) struct CliOptions {
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) path: String,
    pub(crate) verbose: bool,
    pub(crate) source: String,
    pub(crate) camera_name: Option<String>,
    pub(crate) exposure_time_us: Option<u32>,
    pub(crate) analogue_gain: Option<f32>,
    pub(crate) brightness: Option<f32>,
    pub(crate) contrast: Option<f32>,
    pub(crate) saturation: Option<f32>,
    pub(crate) sharpness: Option<f32>,
    pub(crate) device_path: Option<String>,
    pub(crate) pattern: Option<String>,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) framerate: u32,
    pub(crate) bitrate: u32,
}

impl Default for CliOptions {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8554,
            path: "/stream".to_string(),
            verbose: false,
            source: "auto".to_string(),
            camera_name: None,
            exposure_time_us: None,
            analogue_gain: None,
            brightness: None,
            contrast: None,
            saturation: None,
            sharpness: None,
            device_path: None,
            pattern: Some("ball".to_string()),
            width: 1280,
            height: 720,
            framerate: 20,
            bitrate: 2_000_000,
        }
    }
}

pub(crate) fn parse_env_args() -> Result<CliOptions, String> {
    parse_args(std::env::args().skip(1))
}

pub(crate) fn parse_args<I>(args: I) -> Result<CliOptions, String>
where
    I: IntoIterator<Item = String>,
{
    let mut options = CliOptions::default();
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_usage();
                process::exit(0);
            }
            "--host" => options.host = next_value(&mut args, "--host")?,
            "--port" => options.port = parse_value(&mut args, "--port")?,
            "--path" => options.path = next_value(&mut args, "--path")?,
            "--verbose" => options.verbose = true,
            "--source" => options.source = next_value(&mut args, "--source")?,
            "--camera-name" => options.camera_name = Some(next_value(&mut args, "--camera-name")?),
            "--exposure-time-us" => {
                options.exposure_time_us = Some(parse_value(&mut args, "--exposure-time-us")?)
            }
            "--analogue-gain" => {
                options.analogue_gain = Some(parse_value(&mut args, "--analogue-gain")?)
            }
            "--brightness" => options.brightness = Some(parse_value(&mut args, "--brightness")?),
            "--contrast" => options.contrast = Some(parse_value(&mut args, "--contrast")?),
            "--saturation" => options.saturation = Some(parse_value(&mut args, "--saturation")?),
            "--sharpness" => options.sharpness = Some(parse_value(&mut args, "--sharpness")?),
            "--device-path" => options.device_path = Some(next_value(&mut args, "--device-path")?),
            "--pattern" => options.pattern = Some(next_value(&mut args, "--pattern")?),
            "--width" => options.width = parse_value(&mut args, "--width")?,
            "--height" => options.height = parse_value(&mut args, "--height")?,
            "--framerate" => options.framerate = parse_value(&mut args, "--framerate")?,
            "--bitrate" => options.bitrate = parse_value(&mut args, "--bitrate")?,
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    Ok(options)
}

fn next_value<I>(args: &mut I, option: &str) -> Result<String, String>
where
    I: Iterator<Item = String>,
{
    args.next()
        .ok_or_else(|| format!("missing value for {option}"))
}

fn parse_value<T, I>(args: &mut I, option: &str) -> Result<T, String>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
    I: Iterator<Item = String>,
{
    let value = next_value(args, option)?;
    value
        .parse::<T>()
        .map_err(|error| format!("invalid value for {option}: {error}"))
}

fn print_usage() {
    println!(concat!(
        "Usage: cargo run -- [options]\n\n",
        "Options:\n",
        "  --host HOST              destination host. default: 127.0.0.1\n",
        "  --port PORT              RTSP listen port. default: 8554\n",
        "  --path PATH              RTSP mount path. default: /stream\n",
        "  --verbose                show detailed startup fallback diagnostics\n",
        "  --source SOURCE          auto | imx500 | libcamera | v4l2 | videotest. default: auto\n",
        "  --camera-name NAME       camera name for imx500/libcamera\n",
        "  --exposure-time-us USEC  imx500 exposure override in usec\n",
        "  --analogue-gain VALUE    imx500 analogue gain override\n",
        "  --brightness VALUE       imx500 brightness override\n",
        "  --contrast VALUE         imx500 contrast override\n",
        "  --saturation VALUE       imx500 saturation override\n",
        "  --sharpness VALUE        imx500 sharpness override\n",
        "  --device-path PATH       device path for v4l2. example: /dev/video0\n",
        "  --pattern NAME           videotestsrc pattern. default: ball\n",
        "  --width PIXELS           default: 1280\n",
        "  --height PIXELS          default: 720\n",
        "  --framerate FPS          default: 20\n",
        "  --bitrate BPS            default: 2000000\n",
        "  -h, --help               show this help\n\n",
        "Auto selection order:\n",
        "  imx500 -> libcamera -> v4l2 -> videotest\n\n",
        "Examples:\n",
        "  cargo run --\n",
        "  cargo run -- --source videotest --host 127.0.0.1 --port 8554 --path /test\n",
        "  cargo run -- --source v4l2 --device-path /dev/video0 --host 0.0.0.0 --port 8554 --path /camera\n",
        "  cargo run -- --source imx500 --host 0.0.0.0 --port 8554 --path /camera\n",
        "  cargo run -- --source imx500 --exposure-time-us 10000 --analogue-gain 2.0 --contrast 1.1 --host 0.0.0.0 --port 8554 --path /camera\n",
        "  cargo run -- --source libcamera --camera-name /base/soc/i2c0mux/i2c@1/imx500@1a --host 0.0.0.0 --port 8554 --path /camera\n"
    ));
}

#[cfg(test)]
mod tests {
    use super::parse_args;

    #[test]
    fn parse_args_reads_imx500_tuning_options() {
        let options = parse_args([
            "--source".to_string(),
            "imx500".to_string(),
            "--exposure-time-us".to_string(),
            "10000".to_string(),
            "--analogue-gain".to_string(),
            "2.0".to_string(),
            "--brightness".to_string(),
            "0.1".to_string(),
            "--contrast".to_string(),
            "1.1".to_string(),
            "--saturation".to_string(),
            "1.2".to_string(),
            "--sharpness".to_string(),
            "0.8".to_string(),
        ])
        .expect("parse_args should succeed");

        assert_eq!(options.source, "imx500");
        assert_eq!(options.exposure_time_us, Some(10_000));
        assert_eq!(options.analogue_gain, Some(2.0));
        assert_eq!(options.brightness, Some(0.1));
        assert_eq!(options.contrast, Some(1.1));
        assert_eq!(options.saturation, Some(1.2));
        assert_eq!(options.sharpness, Some(0.8));
    }

    #[test]
    fn parse_args_reads_verbose_flag() {
        let options = parse_args(["--verbose".to_string()]).expect("parse_args should succeed");

        assert!(options.verbose);
    }
}
