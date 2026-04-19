/**
 * Chat & Agent module.
 */
window.CHAT = (() => {
    let isAiTyping = false;

    const escapeHtml = (text) => `${text || ''}`
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;');

    const renderInlineMarkdown = (text) => {
        let out = escapeHtml(text);
        out = out.replace(/`([^`]+)`/g, '<code class="md-inline-code">$1</code>');
        out = out.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');
        out = out.replace(/\*([^*]+)\*/g, '<em>$1</em>');
        return out;
    };

    const renderMarkdown = (raw) => {
        const source = `${raw || ''}`;
        const parts = source.split(/```/);
        const html = parts
            .map((part, idx) => {
                if (idx % 2 === 1) {
                    return `<pre class="md-pre"><code>${escapeHtml(part.trim())}</code></pre>`;
                }

                const lines = part.split(/\r?\n/);
                let inList = false;
                const buffer = [];
                lines.forEach((line) => {
                    const bullet = line.match(/^\s*[-*]\s+(.+)$/);
                    if (bullet) {
                        if (!inList) {
                            inList = true;
                            buffer.push('<ul class="md-list">');
                        }
                        buffer.push(`<li>${renderInlineMarkdown(bullet[1])}</li>`);
                        return;
                    }
                    if (inList) {
                        buffer.push('</ul>');
                        inList = false;
                    }
                    if (!line.trim()) {
                        buffer.push('<br>');
                        return;
                    }
                    const heading = line.match(/^\s{0,3}#{1,6}\s+(.+)$/);
                    if (heading) {
                        buffer.push(`<p class="md-heading">${renderInlineMarkdown(heading[1])}</p>`);
                        return;
                    }
                    buffer.push(`<p>${renderInlineMarkdown(line)}</p>`);
                });
                if (inList) buffer.push('</ul>');
                return buffer.join('');
            })
            .join('');

        return html || '<p>-</p>';
    };

    const appendChatMsg = (text, sender) => {
        const chatMessages = document.getElementById('chatMessages');
        if (!chatMessages) return;

        const msgDiv = document.createElement('div');
        if (sender === 'user') {
            msgDiv.className = 'flex w-full mt-2 space-x-3 max-w-xs ml-auto justify-end';
            msgDiv.innerHTML = `
                <div class="p-3 bg-emerald-600/30 rounded-xl msg-user leading-relaxed break-words border border-emerald-500/20">
                    ${escapeHtml(text).replace(/\n/g, '<br>')}
                </div>
            `;
        } else if (sender === 'ai') {
            msgDiv.className = 'flex w-full mt-2 space-x-3 max-w-xs';
            msgDiv.innerHTML = `
                <div class="p-3 bg-slate-800/80 rounded-xl msg-ai leading-relaxed border border-white/5 markdown-body">
                    ${renderMarkdown(text)}
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
            // no-op
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
        if (window.UI.AI.isTyping) return;

        const input = document.getElementById('chatInput');
        const msg = input.value.trim();
        if (!msg) return;

        window.UI.AI.addMessage('user', msg);
        input.value = '';

        window.UI.AI.isTyping = true;
        window.UI.AI.showLoading();

        try {
            // Stack current custom instructions from sidebar config if any
            const fullPrompt = `${window.UI.AI.customInstruction}\n\nClient Input: ${msg}`;
            const reply = await sendMessageToOpenClaw(fullPrompt);
            window.UI.AI.hideLoading();
            window.UI.AI.addMessage('ai', reply);
        } catch (err) {
            window.UI.AI.hideLoading();
            window.UI.AI.addMessage('ai', `服务暂时离线: ${err.message}`);
        } finally {
            window.UI.AI.isTyping = false;
        }
    };

    return {
        handleSubmit,
        sendMessageToOpenClaw,
        renderMarkdown,
        escapeHtml
    };
})();
