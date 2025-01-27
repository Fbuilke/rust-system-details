use std::ffi::OsStr;
use std::path::Path;
use std::thread;
use std::time::Duration;
use nvml_wrapper::Nvml;
use nvml_wrapper::enum_wrappers;
use reqwest::Client;

use sysinfo::{CpuRefreshKind, DiskKind, Disks, RefreshKind, System};
use tokio::runtime::Runtime;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
struct Alarm {
    alarmContent: String,
    alarmDesc: String,
    alarmId: String,
    alarmLevelNo: String,
    alarmLevelNoDesc: String,
    alarmType: String,
    alarmTypeDesc: String,
    precaution: String,
    publishTime: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Index {
    abbreviation: String,
    alias: String,
    content: String,
    level: String,
    name: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Pm25 {
    advice: String,
    aqi: String,
    citycount: i32,
    cityrank: i32,
    co: String,
    color: String,
    level: String,
    no2: String,
    o3: String,
    pm10: String,
    pm25: String,
    quality: String,
    so2: String,
    timestamp: String,
    upDateTime: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Realtime {
    img: String,
    sD: String,
    sendibleTemp: String,
    temp: String,
    time: String,
    wD: String,
    wS: String,
    weather: String,
    ziwaixian: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct WeatherDetailsInfo {
    publishTime: String,
    weather3HoursDetailsInfos: Vec<Weather3HoursDetailsInfo>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Weather3HoursDetailsInfo {
    endTime: String,
    highestTemperature: String,
    img: String,
    isRainFall: String,
    lowerestTemperature: String,
    precipitation: String,
    startTime: String,
    wd: String,
    weather: String,
    ws: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Weather {
    aqi: String,
    date: String,
    img: String,
    sun_down_time: String,
    sun_rise_time: String,
    temp_day_c: String,
    temp_day_f: String,
    temp_night_c: String,
    temp_night_f: String,
    wd: String,
    weather: String,
    week: String,
    ws: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct ApiResponse {
    code: String,
    message: String,
    redirect: String,
    value: Vec<Value>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Value {
    alarms: Vec<Alarm>,
    city: String,
    cityid: i32,
    indexes: Vec<Index>,
    pm25: Pm25,
    provinceName: String,
    realtime: Realtime,
    weatherDetailsInfo: WeatherDetailsInfo,
    weathers: Vec<Weather>,
}

#[derive(Debug)]
struct GpuInfo {
    name: String,
    num_cores: u32,
    memory_bus_width: u32,
    core_clock: u32,
    memory_clock: u32,
    gpu_temperature: u32,
    power_usage: f64,
    power_limit: u32,
    memory_used: f64,
    memory_total: f64,
}

fn get_gpu_info() -> Result<GpuInfo, nvml_wrapper::error::NvmlError> {
    let nvml = Nvml::init()?;
    let device = nvml.device_by_index(0)?;
    let power_limit = device.enforced_power_limit()?;
    let memory_info = device.memory_info()?;
    let power_usage = device.power_usage()?;
    let gpu_temperature = device.temperature(enum_wrappers::device::TemperatureSensor::Gpu)?;
    let core_clock = device.clock(enum_wrappers::device::Clock::Graphics, enum_wrappers::device::ClockId::Current)?;
    let memory_clock = device.clock(enum_wrappers::device::Clock::Memory, enum_wrappers::device::ClockId::Current)?;
    let name = device.name()?;
    let num_cores = device.num_cores()?;
    let memory_bus_width = device.memory_bus_width()?;

    Ok(GpuInfo {
        name,
        num_cores,
        memory_bus_width,
        core_clock,
        memory_clock,
        gpu_temperature,
        power_usage: power_usage as f64 / 1000.0,
        power_limit: power_limit / 1000,
        memory_used: memory_info.used as f64 / (1024.0 * 1024.0 * 1024.0),
        memory_total: memory_info.total as f64 / (1024.0 * 1024.0 * 1024.0),
    })
}

#[derive(Debug)]
struct SystemInfo {
    total_memory: f64,
    used_memory: f64,
    total_swap: f64,
    used_swap: f64,
    system_name: Option<String>,
    kernel_version: Option<String>,
    os_version: Option<String>,
    host_name: Option<String>,
    uptime: u64,
    disks: Vec<DiskInfo>,
    average_cpu_usage: f32,
}

#[derive(Debug)]
struct DiskInfo {
    name: String,
    kind: DiskKind,
    file_system: String,
    mount_point: String,
    total_space: f64,
    available_space: f64,
}

fn get_system_info() -> SystemInfo {
    let mut sys = System::new_all();
    sys.refresh_all();

    let total_memory = bytes_to_gb(sys.total_memory());
    let used_memory = bytes_to_gb(sys.used_memory());
    let total_swap = bytes_to_gb(sys.total_swap());
    let used_swap = bytes_to_gb(sys.used_swap());

    let system_name = System::name();
    let kernel_version = System::kernel_version();
    let os_version = System::os_version();
    let host_name = System::host_name();

    let uptime = System::uptime();
    let disks = get_disk_info();

    let mut s = System::new_with_specifics(RefreshKind::new().with_cpu(CpuRefreshKind::everything()));

    thread::sleep(Duration::from_secs(1));
    s.refresh_cpu();

    let mut total_cpu_usage = 0.0;
    for cpu in s.cpus() {
        total_cpu_usage += cpu.cpu_usage();
    }
    let average_cpu_usage = if !s.cpus().is_empty() {
        total_cpu_usage / s.cpus().len() as f32
    } else {
        0.0
    };

    SystemInfo {
        total_memory,
        used_memory,
        total_swap,
        used_swap,
        system_name,
        kernel_version,
        os_version,
        host_name,
        uptime,
        disks,
        average_cpu_usage,
    }
}

fn os_str_to_option_string(os_str: Option<&OsStr>) -> Option<String> {
    os_str.map(|s| s.to_string_lossy().into_owned())
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn get_disk_info() -> Vec<DiskInfo> {
    let mut disk_info_list: Vec<DiskInfo> = Vec::new();

    let disks = Disks::new_with_refreshed_list();
    for disk in disks.list() {
        let disk_info = DiskInfo {
            name: os_str_to_option_string(Option::from(disk.name())).unwrap(),
            kind: Option::from(disk.kind()).unwrap(),
            file_system: os_str_to_option_string(Option::from(disk.file_system())).unwrap(),
            mount_point: path_to_string(disk.mount_point()).into(),
            total_space: bytes_to_gb(disk.total_space()),
            available_space: bytes_to_gb(disk.available_space()),
        };
        disk_info_list.push(disk_info);
    }

    disk_info_list
}

fn main() {
    match get_gpu_info() {
        Ok(gpu_info) => {
            println!("GPU Name: {}", gpu_info.name);
            println!("Number of Cores: {}", gpu_info.num_cores);
            println!("Memory Bus Width: {}-bit bus width", gpu_info.memory_bus_width);
            println!("GPU Core Clocks: {} MHz", gpu_info.core_clock);
            println!("GPU Memory Clock: {} MHz", gpu_info.memory_clock);
            println!("GPU Temperature: {} C", gpu_info.gpu_temperature);
            println!("Power Usage: {} W", gpu_info.power_usage);
            println!("Power Limit: {} W", gpu_info.power_limit);
            println!("Memory Used: {:.2} GB", gpu_info.memory_used);
            println!("Memory Total: {:.2} GB", gpu_info.memory_total);
        }
        Err(e) => println!("Error: {}", e),
    }

    let system_info = get_system_info();

    println!("=> system:");
    println!("Total Memory: {:.2} GB", system_info.total_memory);
    println!("Used Memory: {:.2} GB", system_info.used_memory);
    println!("Total Swap: {:.2} GB", system_info.total_swap);
    println!("Used Swap: {:.2} GB", system_info.used_swap);
    println!("System Name: {:?}", system_info.system_name);
    println!("Kernel Version: {:?}", system_info.kernel_version);
    println!("OS Version: {:?}", system_info.os_version);
    println!("Host Name: {:?}", system_info.host_name);
    let (days, hours, minutes, remaining_seconds) = convert_seconds(system_info.uptime);
    println!("Uptime {} seconds is equivalent to {} days, {} hours, {} minutes, and {} seconds", System::uptime(), days, hours, minutes, remaining_seconds);
    println!("Average CPU Usage: {:.2}%", system_info.average_cpu_usage);

    println!("=> disks:");
    for disk in system_info.disks {
        println!(
            "{:?}\t{:?}\t{:?}\t{:?}\t{:.2} GB\t{:.2} GB",
            disk.name,
            disk.kind,
            disk.file_system,
            disk.mount_point,
            disk.total_space,
            disk.available_space
        );
    }


    let mut rtt =  Runtime::new().unwrap();
    rtt.block_on(async {
        // 创建一个HTTP客户端
        let client = Client::new();

        // 发送GET请求并等待响应
        let response = client
            .get("https://api.oioweb.cn/api/weather/GetWeather")
            .send()
            .await
            .unwrap();

        // 检查响应状态码
        if response.status().is_success() {
            // 读取响应的内容
            let body = response.text().await.unwrap();
            println!("请求成功: {}", body);
        } else {
            println!("请求失败: {}", response.status());
        }
    });

    // 创建一个运行时环境
    let mut rt = Runtime::new().unwrap();

    // 在运行时环境中执行异步任务
    rt.block_on(async {
        // 创建一个HTTP客户端
        let client = Client::new();

        // 发送GET请求并等待响应
        let response = client
            .get("https://aider.meizu.com/app/weather/listWeather?cityIds=101200105")
            .send()
            .await
            .unwrap();

        // 检查响应状态码
        if response.status().is_success() {
            // 读取响应的内容
            let body = response.text().await.unwrap();

            // 打印返回的数据
            let response: ApiResponse = serde_json::from_str(body.as_str()).unwrap();

            println!("Code: {}", response.code);
            println!("Message: {}", response.message);
            println!("Redirect: {}", response.redirect);

            for value in response.value {
                for alarm in value.alarms {
                    println!("Alarm Content: {}", alarm.alarmContent);
                    println!("Alarm Description: {}", alarm.alarmDesc);
                    println!("Alarm ID: {}", alarm.alarmId);
                    println!("Alarm Level: {}", alarm.alarmLevelNoDesc);
                    println!("Alarm Type: {}", alarm.alarmTypeDesc);
                    println!("Precaution: {}", alarm.precaution);
                    println!("Publish Time: {}", alarm.publishTime);
                    println!("------------------------");
                }

                println!("City: {}", value.city);
                println!("City ID: {}", value.cityid);

                for index in value.indexes {
                    println!("Index Name: {}", index.name);
                    println!("Index Level: {}", index.level);
                    println!("Index Content: {}", index.content);
                    println!("------------------------");
                }

                println!("PM2.5 Quality: {}", value.pm25.quality);
                println!("PM2.5 AQI: {}", value.pm25.aqi);

                println!("Province Name: {}", value.provinceName);

                println!("Realtime Weather: {}", value.realtime.weather);
                println!("Realtime Temperature: {}", value.realtime.temp);
                println!("Realtime Wind: {} {}", value.realtime.wD, value.realtime.wS);

                for weather in value.weathers {
                    println!("Weather Date: {}", weather.date);
                    println!("Weather: {}", weather.weather);
                    println!("Day Temperature: {}", weather.temp_day_c);
                    println!("Night Temperature: {}", weather.temp_night_c);
                    println!("------------------------");
                }
            }
        } else {
            println!("请求失败: {}", response.status());
        }
    });
}

fn bytes_to_gb(bytes: u64) -> f64 {
    // 1 GB = 1024^3 bytes
    let gb = bytes as f64 / 1024_f64.powi(3);
    gb
}

fn convert_seconds(seconds: u64) -> (u64, u64, u64, u64) {
    let days = seconds / (24 * 3600);
    let hours = (seconds / 3600) % 24;
    let minutes = (seconds / 60) % 60;
    let remaining_seconds = seconds % 60;

    (days, hours, minutes, remaining_seconds)
}