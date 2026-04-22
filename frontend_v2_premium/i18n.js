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
            'modal_preview': '图传预览', 'img_fail': '图片加载失败'
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
            'modal_preview': 'Image Preview', 'img_fail': 'Image Failed to Load'
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
            'modal_preview': 'Pratonton Imej', 'img_fail': 'Imej Gagal Dimuatkan'
        }
    };

    const updateDOM = () => {
        document.querySelectorAll('[data-i18n]').forEach(el => {
            const key = el.getAttribute('data-i18n');
            if (DICT[currentLang][key]) {
                if (el.tagName === 'INPUT' && el.type === 'text') {
                    el.placeholder = DICT[currentLang][key];
                } else {
                    el.textContent = DICT[currentLang][key];
                }
            }
        });
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
