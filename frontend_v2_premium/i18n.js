/**
 * Lightweight localization (I18n) core.
 */
window.I18N = (() => {
    let currentLang = localStorage.getItem('agri_lang') || 'zh';

    const DICT = {
        'zh': {
            'nav_home': '首页', 'nav_charts': '图表', 'nav_health': '设备健康度', 'nav_refresh': '刷新系统',
            'sys_running': '系统运行中', 'cloud_status': '云端服务器状态', 'support_online': '在线协助中',
            'chat_placeholder': '发送指令...', 'crop_select': '作物种类选择', 'loc_select': '位置选择', 'all_locations': '全部位置',
            'field_details': '字段明细', 'no_data': '无数据', 'processing': '处理中', 'disease_rate': '患病率',
            'vision_timeline': '视觉观测时间轴', 'mean_value': '平均值', 'syncing': '同步中...',
            'error_time': '错误：起始时间不能晚于结束时间', 'no_history': '所选时间范围内无历史数据', 'back_home': '返回主页',
            'select_sensors': '选择传感器', 'farm_positioning': '农场数据定位', 'loading': '加载中...',
            'gateway_vision': '网关实时视觉', 'live_streaming': '实时推流中...', 'ai_feedback': '视觉 AI 反馈',
            'sensor_base': '网关传感器设备底座', 'new_session': '新建会话', 'session_history': '会话历史',
            'clear_all': '清除全部', 'immersive_chat': '沉浸式对话站', 'back_overview': '返回概览',
            'modal_preview': '图传预览', 'img_fail': '图片加载失败',
            'chart_start_time': '起始时间', 'chart_end_time': '截止时间', 'sync_board': '同步看板',
            'select_analysis_hint': '请选择分析条件并点击“同步看板”', 'node_mesh': '节点传感器网格',
            'config_sensors': '配置节点传感器', 'vision_sync': '视觉同步', 'send_msg': '发送消息',
            'chat_main_placeholder': '在此输入深度指令或技术咨询...', 'agent_skill_title': 'AI-ag Agent Skill (协作版)',
            'agent_skill_desc': '包含服务巡检、日志定位、数据库检查、网关管理等核心权限集。点击查看完整协议。',
            'enhanced_instr': '增强指令集 (User Override)', 'add_instr_placeholder': '添加新指令...',
            'token_usage': 'Token 消耗量', 'load_normal': '流量负载正常', 'crop_sector_analysis': '作物区块分析',
            'cloud_ops': '云端运维', 'gateway_mesh': '网关拓扑', 'sensor_array': '传感器阵列',
            'sensor_lab': '传感器实验室', 'hardware_monitor_desc': '底层硬件寄存器实时探测与协议栈监控',
            'agent_skill_protocol': 'AI-ag Agent Skill 核心协议',
            'server_detail_gateway': 'Edge-Cloud 实时链路', 'server_detail_ai': '视觉语义分析引擎',
            'server_detail_db': '时序数据库集群', 'server_detail_cdn': '多媒体分发链路同步滞后'
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
            'server_detail_db': 'Time-series Database Cluster', 'server_detail_cdn': 'Multimedia distribution link sync lag'
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
            'server_detail_db': 'Kluster Pangkalan Data Siri-Masa', 'server_detail_cdn': 'Lat penyegerakan pautan pengedaran multimedia'
        }
    };

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
