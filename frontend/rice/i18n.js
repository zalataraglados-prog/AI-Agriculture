/**
 * Lightweight localization (I18n) core.
 */
window.I18N = (() => {
    let currentLang = localStorage.getItem('agri_lang') || 'zh';

    const DICT = {
        'zh': {
            'nav_home': '棣栭〉', 'nav_charts': '鍥捐〃', 'nav_health': '璁惧鍋ュ悍搴?, 'nav_refresh': '鍒锋柊绯荤粺',
            'sys_running': '绯荤粺杩愯涓?, 'cloud_status': '浜戠鏈嶅姟鍣ㄧ姸鎬?, 'support_online': '鍦ㄧ嚎鍗忓姪涓?,
            'chat_placeholder': '鍙戦€佹寚浠?..', 'crop_select': '浣滅墿绉嶇被閫夋嫨', 'loc_select': '浣嶇疆閫夋嫨', 'all_locations': '鍏ㄩ儴浣嶇疆',
            'field_details': '瀛楁鏄庣粏', 'no_data': '鏃犳暟鎹?, 'processing': '澶勭悊涓?, 'disease_rate': '鎮ｇ梾鐜?,
            'vision_timeline': '瑙嗚瑙傛祴鏃堕棿杞?, 'mean_value': '骞冲潎鍊?, 'syncing': '鍚屾涓?..',
            'error_time': '閿欒锛氳捣濮嬫椂闂翠笉鑳芥櫄浜庣粨鏉熸椂闂?, 'no_history': '鎵€閫夋椂闂磋寖鍥村唴鏃犲巻鍙叉暟鎹?, 'back_home': '杩斿洖涓婚〉',
            'select_sensors': '閫夋嫨浼犳劅鍣?, 'farm_positioning': '鍐滃満鏁版嵁瀹氫綅', 'loading': '鍔犺浇涓?..',
            'gateway_vision': '缃戝叧瀹炴椂瑙嗚', 'live_streaming': '瀹炴椂鎺ㄦ祦涓?..', 'ai_feedback': '瑙嗚 AI 鍙嶉',
            'sensor_base': '缃戝叧浼犳劅鍣ㄨ澶囧簳搴?, 'new_session': '鏂板缓浼氳瘽', 'session_history': '浼氳瘽鍘嗗彶',
            'clear_all': '娓呴櫎鍏ㄩ儴', 'immersive_chat': '娌夋蹈寮忓璇濈珯', 'back_overview': '杩斿洖姒傝',
            'modal_preview': '鍥句紶棰勮', 'img_fail': '鍥剧墖鍔犺浇澶辫触',
            'chart_start_time': '璧峰鏃堕棿', 'chart_end_time': '鎴鏃堕棿', 'sync_board': '鍚屾鐪嬫澘',
            'select_analysis_hint': '璇烽€夋嫨鍒嗘瀽鏉′欢骞剁偣鍑烩€滃悓姝ョ湅鏉库€?, 'node_mesh': '鑺傜偣浼犳劅鍣ㄧ綉鏍?,
            'config_sensors': '閰嶇疆鑺傜偣浼犳劅鍣?, 'vision_sync': '瑙嗚鍚屾', 'send_msg': '鍙戦€佹秷鎭?,
            'chat_main_placeholder': '鍦ㄦ杈撳叆娣卞害鎸囦护鎴栨妧鏈挩璇?..', 'agent_skill_title': 'AI-ag Agent Skill (鍗忎綔鐗?',
            'agent_skill_desc': '鍖呭惈鏈嶅姟宸℃銆佹棩蹇楀畾浣嶃€佹暟鎹簱妫€鏌ャ€佺綉鍏崇鐞嗙瓑鏍稿績鏉冮檺闆嗐€傜偣鍑绘煡鐪嬪畬鏁村崗璁€?,
            'enhanced_instr': '澧炲己鎸囦护闆?(User Override)', 'add_instr_placeholder': '娣诲姞鏂版寚浠?..',
            'token_usage': 'Token 娑堣€楅噺', 'load_normal': '娴侀噺璐熻浇姝ｅ父', 'crop_sector_analysis': '浣滅墿鍖哄潡鍒嗘瀽',
            'cloud_ops': '浜戠杩愮淮', 'gateway_mesh': '缃戝叧鎷撴墤', 'sensor_array': '浼犳劅鍣ㄩ樀鍒?,
            'sensor_lab': '浼犳劅鍣ㄥ疄楠屽', 'hardware_monitor_desc': '搴曞眰纭欢瀵勫瓨鍣ㄥ疄鏃舵帰娴嬩笌鍗忚鏍堢洃鎺?,
            'agent_skill_protocol': 'AI-ag Agent Skill 鏍稿績鍗忚',
            'server_detail_gateway': 'Edge-Cloud 瀹炴椂閾捐矾', 'server_detail_ai': '瑙嗚璇箟鍒嗘瀽寮曟搸',
            'server_detail_db': '鏃跺簭鏁版嵁搴撻泦缇?, 'server_detail_cdn': '澶氬獟浣撳垎鍙戦摼璺悓姝ユ粸鍚?,
            'sensor_status': '浼犳劅鍣ㄧ姸鎬?, 'status_ok': '姝ｅ父', 'status_fault': '鏁呴殰', 'status_online': '鍦ㄧ嚎'
        },
        'en': {
            'nav_home': 'Dashboard', 'nav_charts': 'Analytics', 'nav_health': 'System Health', 'nav_refresh': 'Refresh System',
            'sys_running': 'System Active', 'cloud_status': 'Cloud Server Node Status', 'support_online': 'Online Assisting',
            'chat_placeholder': 'Send command...', 'crop_select': 'Select Crop', 'loc_select': 'Select Location', 'all_locations': 'All Locations',
            'field_details': 'Field Diagnostics', 'no_data': 'No Data', 'processing': 'Processing', 'disease_rate': 'Disease Rate',
            'vision_timeline': 'Vision Analytics Timeline', 'mean_value': 'Mean Value', 'syncing': 'SYNCHRONIZING...',
            'error_time': 'Error: Start time cannot be later than end time', 'no_history': 'No historical data in selected range', 'back_home': 'Back to Dashboard',
            'select_sensors': 'Select Sensors', 'farm_positioning': 'Farm Positioning Data', 'loading': 'Loading...',
            'gateway_vision': 'Gateway Live Vision', 'live_streaming': 'Live Streaming...', 'ai_feedback': 'Vision AI Feedback',
            'sensor_base': 'Gateway Sensor Base', 'new_session': 'New Session', 'session_history': 'Session History',
            'clear_all': 'Clear All', 'immersive_chat': 'Immersive Chat Station', 'back_overview': 'Back to Overview',
            'modal_preview': 'Image Preview', 'img_fail': 'Image Failed to Load',
            'chart_start_time': 'Start Time', 'chart_end_time': 'End Time', 'sync_board': 'Sync Dashboard',
            'select_analysis_hint': 'Please select analysis criteria and click "Sync Dashboard"', 'node_mesh': 'Node Sensor Mesh',
            'config_sensors': 'Configure Node Sensors', 'vision_sync': 'Vision Sync', 'send_msg': 'Send Message',
            'chat_main_placeholder': 'Enter deep commands or technical inquiries...', 'agent_skill_title': 'AI-ag Agent Skill (Collaboration)',
            'agent_skill_desc': 'Includes service inspection, log positioning, database check, gateway management, etc. Click to view protocol.',
            'enhanced_instr': 'Enhanced Instructions (User Override)', 'add_instr_placeholder': 'Add new instruction...',
            'token_usage': 'Token Usage', 'load_normal': 'Load normal', 'crop_sector_analysis': 'Crop Sector Analysis',
            'cloud_ops': 'Cloud Operations', 'gateway_mesh': 'Gateway Mesh', 'sensor_array': 'Sensor Array',
            'sensor_lab': 'Sensor Laboratory', 'hardware_monitor_desc': 'Low-level hardware register detection and stack monitoring',
            'agent_skill_protocol': 'AI-ag Agent Skill Core Protocol',
            'server_detail_gateway': 'Edge-Cloud Real-time Link', 'server_detail_ai': 'Vision Semantic Analysis Engine',
            'server_detail_db': 'Time-series Database Cluster', 'server_detail_cdn': 'Multimedia distribution link sync lag',
            'sensor_status': 'Sensor Status', 'status_ok': 'OK', 'status_fault': 'FAULT', 'status_online': 'ONLINE'
        },
        'ms': {
            'nav_home': 'Laman Utama', 'nav_charts': 'Analisis', 'nav_health': 'Kesihatan Peranti', 'nav_refresh': 'Segar Semula',
            'sys_running': 'Sistem Aktif', 'cloud_status': 'Status Pelayan Awan', 'support_online': 'Bantuan Dalam Talian',
            'chat_placeholder': 'Hantar arahan...', 'crop_select': 'Pilih Tanaman', 'loc_select': 'Pilih Lokasi', 'all_locations': 'Semua Lokasi',
            'field_details': 'Butiran Medan', 'no_data': 'Tiada Data', 'processing': 'Sedang Diproses', 'disease_rate': 'Kadar Penyakit',
            'vision_timeline': 'Garis Masa Visual', 'mean_value': 'Nilai Min', 'syncing': 'SEDANG MENYEGERAK...',
            'error_time': 'Ralat: Masa mula tidak boleh lewat', 'no_history': 'Tiada data sejarah dalam julat ini', 'back_home': 'Kembali',
            'select_sensors': 'Pilih Penderia', 'farm_positioning': 'Data Kedudukan Ladang', 'loading': 'Memuatkan...',
            'gateway_vision': 'Visi Langsung Gerbang', 'live_streaming': 'Penstriman Langsung...', 'ai_feedback': 'Maklum Balas AI Visual',
            'sensor_base': 'Pangkalan Penderia Gerbang', 'new_session': 'Sesi Baharu', 'session_history': 'Sejarah Sesi',
            'clear_all': 'Kosongkan Semua', 'immersive_chat': 'Sembang Mengasyikkan', 'back_overview': 'Kembali ke Gambaran Keseluruhan',
            'modal_preview': 'Pratonton Imej', 'img_fail': 'Imej Gagal Dimuatkan',
            'chart_start_time': 'Masa Mula', 'chart_end_time': 'Masa Tamat', 'sync_board': 'Segar Semula Papan',
            'select_analysis_hint': 'Sila pilih kriteria analisis dan klik "Segerakkan Papan"', 'node_mesh': 'Mesh Penderia Nod',
            'config_sensors': 'Konfigurasi Penderia Nod', 'vision_sync': 'Penyegerakan Visual', 'send_msg': 'Hantar Mesej',
            'chat_main_placeholder': 'Masukkan arahan mendalam atau pertanyaan teknikal...', 'agent_skill_title': 'Ejen AI-ag (Kolaborasi)',
            'agent_skill_desc': 'Termasuk pemeriksaan perkhidmatan, kedudukan log, pemeriksaan pangkalan data, pengurusan gerbang, dll.',
            'enhanced_instr': 'Arahan Dipertingkatkan', 'add_instr_placeholder': 'Tambah arahan baharu...',
            'token_usage': 'Penggunaan Token', 'load_normal': 'Muatan normal', 'crop_sector_analysis': 'Analisis Sektor Tanaman',
            'cloud_ops': 'Operasi Awan', 'gateway_mesh': 'Mesh Gerbang', 'sensor_array': 'Susunan Penderia',
            'sensor_lab': 'Makmal Penderia', 'hardware_monitor_desc': 'Pengesanan daftar perkakasan tahap rendah dan pemantauan tindanan',
            'agent_skill_protocol': 'Protokol Teras Ejen AI-ag',
            'server_detail_gateway': 'Pautan Masa Nyata Tepi-Awan', 'server_detail_ai': 'Enjin Analisis Semantik Visi',
            'server_detail_db': 'Kluster Pangkalan Data Siri-Masa', 'server_detail_cdn': 'Lat penyegerakan pautan pengedaran multimedia',
            'sensor_status': 'Status Penderia', 'status_ok': 'OK', 'status_fault': 'RALAT', 'status_online': 'AKTIF'
        }
    };

    const EXTRA_I18N = {
        zh: {
            mobile_upload_title: '鎵嬫満鍥句紶涓婁紶',
            mobile_upload_desc: '浠呯湡瀹為摼璺細鍥剧墖灏嗛€氳繃 /api/v1/image/upload 涓婁紶骞惰Е鍙?AI 鍏ュ簱銆?,
            mobile_camera: '鎵嬫満鎷嶇収',
            mobile_album: '鐩稿唽鍥剧墖',
            mobile_clear: '娓呯┖',
            mobile_upload: '涓婁紶',
            no_image_selected: '鏈€夋嫨鍥剧墖',
            waiting_image: '绛夊緟鍥剧墖...',
            upload_ready: '宸插噯澶囦笂浼犮€?,
            mobile_camera_mobile_only: '鎷嶇収鎸夐挳浠呮敮鎸佹墜鏈虹锛岃鏀圭敤鐩稿唽鍥剧墖銆?,
            mobile_ready_camera: '鎵嬫満绔媿鐓т笂浼犲凡灏辩华銆?,
            mobile_ready_desktop: '妗岄潰妯″紡锛氬凡鍚敤鐩稿唽涓婁紶銆?,
            mobile_select_first: '璇峰厛閫夋嫨涓€寮犲浘鐗囥€?,
            mobile_no_device_id: '鏈壘鍒?device_id锛岃浣跨敤 ?device_id=... 鎵撳紑椤甸潰鎴栧厛娉ㄥ唽璁惧銆?,
            mobile_uploading_for: '姝ｅ湪涓婁紶鍒拌澶?,
            mobile_upload_success: '涓婁紶鎴愬姛',
            mobile_upload_failed: '涓婁紶澶辫触',
            accepted: '宸叉帴鏀?,
        },
        en: {
            mobile_upload_title: 'Mobile Image Upload',
            mobile_upload_desc: 'Real pipeline only: image uploads through /api/v1/image/upload and triggers AI ingestion.',
            mobile_camera: 'Camera',
            mobile_album: 'Album',
            mobile_clear: 'Clear',
            mobile_upload: 'Upload',
            no_image_selected: 'No image selected',
            waiting_image: 'Waiting image...',
            upload_ready: 'Ready for upload.',
            mobile_camera_mobile_only: 'Camera capture is mobile-only. Please use Album.',
            mobile_ready_camera: 'Ready for mobile camera upload.',
            mobile_ready_desktop: 'Desktop mode: album upload enabled.',
            mobile_select_first: 'Please select an image first.',
            mobile_no_device_id: 'No device_id found. Open page with ?device_id=... or register device first.',
            mobile_uploading_for: 'Uploading for',
            mobile_upload_success: 'Upload success',
            mobile_upload_failed: 'Upload failed',
            accepted: 'accepted',
        },
        ms: {
            mobile_upload_title: 'Muat Naik Imej Mudah Alih',
            mobile_upload_desc: 'Rantaian sebenar sahaja: imej dimuat naik melalui /api/v1/image/upload dan mencetuskan kemasukan AI.',
            mobile_camera: 'Kamera',
            mobile_album: 'Album',
            mobile_clear: 'Kosongkan',
            mobile_upload: 'Muat Naik',
            no_image_selected: 'Tiada imej dipilih',
            waiting_image: 'Menunggu imej...',
            upload_ready: 'Sedia untuk muat naik.',
            mobile_camera_mobile_only: 'Tangkapan kamera hanya untuk mudah alih. Sila guna Album.',
            mobile_ready_camera: 'Sedia untuk muat naik kamera mudah alih.',
            mobile_ready_desktop: 'Mod desktop: muat naik album diaktifkan.',
            mobile_select_first: 'Sila pilih imej dahulu.',
            mobile_no_device_id: 'Tiada device_id. Buka dengan ?device_id=... atau daftar peranti dahulu.',
            mobile_uploading_for: 'Memuat naik untuk',
            mobile_upload_success: 'Muat naik berjaya',
            mobile_upload_failed: 'Muat naik gagal',
            accepted: 'diterima',
        },
    };
    Object.keys(EXTRA_I18N).forEach((lang) => {
        if (!DICT[lang]) DICT[lang] = {};
        Object.assign(DICT[lang], EXTRA_I18N[lang]);
    });

    const updateDOM = () => {
        document.querySelectorAll('[data-i18n]').forEach(el => {
            const key = el.getAttribute('data-i18n');
            if (DICT[currentLang][key]) {
                if (['INPUT', 'TEXTAREA'].includes(el.tagName)) {
                    el.placeholder = DICT[currentLang][key];
                } else {
                    el.textContent = DICT[currentLang][key];
                }
            }
        });

        // Update language display in header
        const display = document.getElementById('currentLangDisplay');
        if (display) display.textContent = currentLang.toUpperCase();
        document.documentElement.lang = currentLang === 'zh' ? 'zh-CN' : currentLang;
    };

    return {
        init: () => {
            updateDOM();
        },
        setLanguage: (lang) => {
            if (!DICT[lang]) return;
            currentLang = lang;
            localStorage.setItem('agri_lang', lang);
            updateDOM();
            
            // Re-render UI components that contain dynamic text
            if (typeof window.UI !== 'undefined') {
                if (document.getElementById('view-home').classList.contains('active')) {
                    window.UI.HomePositioning.updateSummary();
                } else if (document.getElementById('view-charts').classList.contains('active')) {
                    window.UI.Charts.refresh();
                }
            }
        },
        getLanguage: () => currentLang,
        t: (key) => {
            return DICT[currentLang][key] || key;
        }
    };
})();

// Attach shortcut to global scope
window.t = window.I18N.t;

document.addEventListener('DOMContentLoaded', () => {
    window.I18N.init();
});
