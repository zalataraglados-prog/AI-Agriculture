pub mod models;
pub mod processor;
pub mod db_manager;

use chrono::{Utc, TimeZone};
use processor::CustomGatewayProcessor;
use db_manager::DbManager;
use models::DataQueryRequest;


fn main() {
    let processor = CustomGatewayProcessor::new();
    let dsn = std::env::var("DATABASE_URL").unwrap_or_else(|_| "host=localhost user=postgres dbname=CICSIC port=5432".to_string());

    let mut manager = match DbManager::new(&dsn) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("[ERROR] 数据库连接失败: {}", e);
            return;
        }
    };

    println!("🚀 开始模拟 IoT 数据写入...");
    let test_data: Vec<&[u8]> = vec![
        b"{ \"device_id\": \"GW_01\", \"time\": \"2023-10-27T10:00:00Z\", \"value\": 25.1, \"status\": \"ok\", \"region\": \"BEIJING\" }",
        b"{ \"device_id\": \"GW_01\", \"time\": \"2023-10-27T11:30:00Z\", \"value\": 26.5, \"status\": \"ok\", \"region\": \"BEIJING\" }",
        b"{ \"device_id\": \"GW_02\", \"time\": \"2023-10-27T12:15:00Z\", \"value\": 22.0, \"status\": \"ok\", \"region\": \"SHANGHAI\" }",
        b"{ \"device_id\": \"CAM_01\", \"file_path\": \"/images/2023/10/27/img_001.jpg\", \"time\": \"2023-10-27T11:05:00Z\", \"object_stamp\": \"apple\", \"region\": \"BEIJING\" }",
    ];

    for raw in test_data {
        if let Some(record) = processor.process(raw) {
            manager.add_data(record);
        }
    }
    manager.flush_all().expect("Flush failed");
    println!("✅ IoT 数据写入完成！\n");

    println!("🤖 模拟 AI 组请求数据...");
    let start_time = Utc.with_ymd_and_hms(2023, 10, 27, 10, 0, 0).unwrap();
    let end_time = Utc.with_ymd_and_hms(2023, 10, 27, 14, 0, 0).unwrap();

    let request = DataQueryRequest {
        start_time,
        end_time,
        device_id: None,
    };
    match manager.get_sensor_chunks(&request) {
        Ok(chunks) => {
            println!("📦 收到 {} 个传感器数据块:", chunks.len());
            for (i, chunk) in chunks.iter().enumerate() {
                println!("   [块 {}] 时间范围: {} -> {}", i + 1, chunk.window_start, chunk.window_end);
                println!("           数据点数: {}", chunk.data.len());
                for d in &chunk.data {
                    println!("             - Dev: {}, Time: {}, Val: {}", d.device_id, d.time, d.value);
                }
            }
        }
        Err(e) => eprintln!("❌ 查询传感器数据失败: {}", e),
    }
    match manager.get_image_chunks(&request) {
        Ok(chunks) => {
            println!("\n📸 收到 {} 个图片数据块:", chunks.len());
            for (i, chunk) in chunks.iter().enumerate() {
                println!("   [块 {}] 时间范围: {} -> {}", i + 1, chunk.window_start, chunk.window_end);
                println!("           图片数量: {}", chunk.data.len());
                for img in &chunk.data {
                    println!("             - Path: {}, Obj: {}", img.file_path, img.object_stamp);
                }
            }
        }
        Err(e) => eprintln!("❌ 查询图片数据失败: {}", e),
    }
    manager.close();
}
