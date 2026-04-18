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
        const response = await fetch('/api/v1/chat', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                message: msg,
                context: {
                    source: 'frontend_v2_premium',
                    ts: new Date().toISOString(),
                },
            }),
        });

        let data = null;
        try {
            data = await response.json();
        } catch (_err) {
            // ignore parse failure and fallback below
        }

        if (!response.ok) {
            const message = data?.message || `chat request failed (${response.status})`;
            throw new Error(message);
        }

        if (!data?.reply || typeof data.reply !== 'string') {
            throw new Error('chat response missing reply');
        }

        return data.reply;
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

        try {
            const reply = await sendMessageToOpenClaw(msg);
            removeChatLoading();
            appendChatMsg(reply, 'ai');
        } catch (err) {
            removeChatLoading();
            appendChatMsg(`AI service unavailable: ${err.message}`, 'ai');
        } finally {
            isAiTyping = false;
        }
    };

    return {
        handleSubmit,
    };
})();
