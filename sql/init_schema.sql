-- 传感器数据表
CREATE TABLE IF NOT EXISTS sensor_data (
                                           id SERIAL PRIMARY KEY,
                                           device_id VARCHAR(50) NOT NULL,
    time TIMESTAMP WITH TIME ZONE NOT NULL,
                       value DOUBLE PRECISION,
                       status VARCHAR(20),
    region_code VARCHAR(20),
    CONSTRAINT unique_sensor_record UNIQUE (device_id, time)
    );

-- 图片索引表
CREATE TABLE IF NOT EXISTS image_index (
                                           id SERIAL PRIMARY KEY,
                                           file_path TEXT NOT NULL,
                                           capture_time TIMESTAMP WITH TIME ZONE NOT NULL,
                                           object_stamp VARCHAR(100),
    region_code VARCHAR(20),
    device_id VARCHAR(50),
    CONSTRAINT unique_image_record UNIQUE (file_path, capture_time)
    );

-- 性能索引
CREATE INDEX IF NOT EXISTS idx_sensor_time ON sensor_data (time DESC);
CREATE INDEX IF NOT EXISTS idx_image_time ON image_index (capture_time DESC);