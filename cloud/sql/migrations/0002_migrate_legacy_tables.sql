DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM information_schema.tables
        WHERE table_schema = 'public' AND table_name = 'sensor_data'
    ) THEN
        INSERT INTO sensor_telemetry (ts, device_id, sensor_id, fields_json)
        SELECT
            sd.time AS ts,
            sd.device_id,
            'legacy_sensor' AS sensor_id,
            jsonb_build_object(
                'value', sd.value,
                'status', COALESCE(sd.status, ''),
                'region_code', COALESCE(sd.region_code, '')
            ) AS fields_json
        FROM sensor_data sd
        ON CONFLICT DO NOTHING;
    END IF;
END $$;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM information_schema.tables
        WHERE table_schema = 'public' AND table_name = 'image_index'
    ) THEN
        INSERT INTO image_uploads (
            upload_id,
            device_id,
            captured_at,
            received_at,
            location,
            crop_type,
            farm_note,
            saved_path,
            sha256,
            image_type,
            file_size,
            upload_status,
            error_message
        )
        SELECT
            md5(COALESCE(ii.file_path, '') || '|' || ii.capture_time::text) AS upload_id,
            COALESCE(NULLIF(ii.device_id, ''), 'legacy_unknown'),
            ii.capture_time AS captured_at,
            ii.capture_time AS received_at,
            COALESCE(ii.region_code, '') AS location,
            '' AS crop_type,
            COALESCE(ii.object_stamp, '') AS farm_note,
            ii.file_path AS saved_path,
            '' AS sha256,
            CASE
                WHEN lower(ii.file_path) LIKE '%.png' THEN 'png'
                ELSE 'jpeg'
            END AS image_type,
            0 AS file_size,
            'stored' AS upload_status,
            NULL AS error_message
        FROM image_index ii
        ON CONFLICT DO NOTHING;
    END IF;
END $$;
