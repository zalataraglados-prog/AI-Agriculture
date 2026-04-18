/**
 * Chat & Agent Module
 * Handles interaction with the OpenClaw Agri-AI.
 */

window.CHAT = (() => {
    let isAiTyping = false;

    const appendChatMsg = (text, sender) => {
        const chatMessages = document.getElementById('chatMessages');
        if (!chatMessages) return;

        const msgDiv = document.createElement('div');
        if (sender === 'user') {
            msgDiv.className = 'flex w-full mt-2 space-x-3 max-w-xs ml-auto justify-end';
            msgDiv.innerHTML = `
                <div class="p-3 bg-emerald-600/30 rounded-xl msg-user leading-relaxed break-words border border-emerald-500/20">
                    ${text}
                </div>
            `;
        } else if (sender === 'ai') {
            msgDiv.className = 'flex w-full mt-2 space-x-3 max-w-xs';
            msgDiv.innerHTML = `
                <div class="p-3 bg-slate-800/80 rounded-xl msg-ai leading-relaxed border border-white/5">
                    ${text}
                </div>
            `;
        } else if (sender === 'loading') {
            msgDiv.id = 'ai-typing-indicator';
            msgDiv.className = 'flex w-full mt-2 space-x-3 max-w-xs';
            msgDiv.innerHTML = `
                <div class="p-3 bg-slate-800/80 rounded-xl msg-ai flex items-center gap-2 border border-white/5">
                    <div class="w-1.5 h-1.5 bg-emerald-400 rounded-full animate-bounce"></div>
                    <div class="w-1.5 h-1.5 bg-emerald-400 rounded-full animate-bounce" style="animation-delay: 0.1s"></div>
                    <div class="w-1.5 h-1.5 bg-emerald-400 rounded-full animate-bounce" style="animation-delay: 0.2s"></div>
                </div>
            `;
        }
        
        chatMessages.appendChild(msgDiv);
        chatMessages.scrollTop = chatMessages.scrollHeight;
    };

    const removeChatLoading = () => {
        const loading = document.getElementById('ai-typing-indicator');
        if (loading) loading.remove();
    };

    const sendMessageToOpenClaw = async (msg) => {
        // Mock API call - in production, this would call /api/v1/chat
        return new Promise((resolve) => {
            setTimeout(() => {
                const lower = msg.toLowerCase();
                if(lower.includes('status') || lower.includes('状态')) resolve("当前所有网关链路探测结果为 [HEALTHY]。Sector 01-A 的 RICE_FIELD 饱和度正常，建议维持现有灌溉频率。");
                else if(lower.includes('sensor') || lower.includes('传感器')) resolve("您的传感器底座目前有 4 个活跃节点。其中 SOIL_MODBUS_02 反馈的肥力区间正常。");
                else resolve("收到指令。我已经准备好对该地块执行深度分析或环境调节。您可以继续询问。");
            }, 1000);
        });
    };

    const handleSubmit = async (e) => {
        if (e) e.preventDefault();
        if (isAiTyping) return;

        const input = document.getElementById('chatInput');
        const msg = input.value.trim();
        if (!msg) return;

        appendChatMsg(msg, 'user');
        input.value = '';
        
        isAiTyping = true;
        appendChatMsg('', 'loading');

        const reply = await sendMessageToOpenClaw(msg);
        
        removeChatLoading();
        appendChatMsg(reply, 'ai');
        isAiTyping = false;
    };

    return {
        handleSubmit
    };
})();
