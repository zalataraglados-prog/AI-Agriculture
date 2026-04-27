/**
 * Chat & Agent module — v2 streaming.
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

    const updateAIMessage = (msgElement, text) => {
        if (!text) return;
        const body = msgElement.querySelector('.markdown-body');
        if (body) {
            body.innerHTML = renderMarkdown(text);
        } else {
            // Fallback: find any inner container
            const inner = msgElement.querySelector('.msg-ai') || msgElement.firstElementChild;
            if (inner) {
                inner.innerHTML = '<div class="markdown-body">' + renderMarkdown(text) + '</div>';
            }
        }
        const chatMessages = document.getElementById('chatMessages');
        if (chatMessages) chatMessages.scrollTop = chatMessages.scrollHeight;
    };

    const sendMessageToOpenClaw = async (msg, onChunk) => {
        const controller = new AbortController();
        const timeout = setTimeout(() => controller.abort(), 120000);

        try {
            const response = await fetch('/api/v1/chat/stream', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    message: msg,
                    session_id: Date.now().toString(36),
                    context: {
                        source: 'frontend_v2_premium',
                        ts: new Date().toISOString(),
                    },
                }),
                signal: controller.signal,
            });

            if (!response.ok) {
                let errMsg = `chat request failed (${response.status})`;
                try { const d = await response.json(); errMsg = d?.message || errMsg; } catch {}
                throw new Error(errMsg);
            }

            const reader = response.body.getReader();
            const decoder = new TextDecoder();
            let buffer = '';
            let fullReply = '';

            while (true) {
                const { done, value } = await reader.read();
                if (done) break;

                buffer += decoder.decode(value, { stream: true });
                const lines = buffer.split('\n');
                buffer = lines.pop() || '';

                for (const line of lines) {
                    if (!line.startsWith('data: ')) continue;
                    try {
                        const chunk = JSON.parse(line.slice(6));
                        if (chunk.done) return fullReply;
                        if (chunk.text) {
                            fullReply += chunk.text;
                            if (typeof onChunk === 'function') onChunk(fullReply);
                        }
                    } catch {}
                }
            }

            return fullReply;
        } finally {
            clearTimeout(timeout);
        }
    };

    const handleSubmit = async (e) => {
        if (e) e.preventDefault();
        if (window.UI.AI.isTyping) return;

        const input = document.getElementById('chatInput');
        const msg = input.value.trim();
        if (!msg) return;

        const chatMessages = document.getElementById('chatMessages');
        if (!chatMessages) return;

        // 1. Append user message directly (avoids renderMessagesByList reset during streaming)
        const userDiv = document.createElement('div');
        userDiv.className = 'flex w-full mt-2 space-x-3 max-w-xs ml-auto justify-end';
        userDiv.innerHTML = '<div class="p-3 bg-emerald-600/30 rounded-xl msg-user leading-relaxed break-words border border-emerald-500/20">' + escapeHtml(msg).replace(/\n/g, '<br>') + '</div>';
        chatMessages.appendChild(userDiv);
        chatMessages.scrollTop = chatMessages.scrollHeight;
        input.value = '';

        window.UI.AI.isTyping = true;

        // 2. Cursor dots
        const aiMsgDiv = document.createElement('div');
        aiMsgDiv.id = 'streaming-ai-msg';
        aiMsgDiv.className = 'flex w-full mt-2 space-x-3 max-w-xs';
        aiMsgDiv.innerHTML = '<div class="p-3 bg-slate-800/80 rounded-xl msg-ai flex items-center gap-2 border border-white/5 markdown-body">' +
            '<span class="chat-cursor-dot"></span><span class="chat-cursor-dot"></span><span class="chat-cursor-dot"></span></div>';
        chatMessages.appendChild(aiMsgDiv);
        chatMessages.scrollTop = chatMessages.scrollHeight;

        try {
            const stack = (window.UI.AI.instructionList || []).join('\n');
            const fullPrompt = stack ? stack + '\n\nClient Input: ' + msg : msg;
            const finalReply = await sendMessageToOpenClaw(fullPrompt, function(reply) {
                updateAIMessage(aiMsgDiv, reply);
            });

            // 3. Remove streaming bubble
            if (aiMsgDiv.parentNode) aiMsgDiv.parentNode.removeChild(aiMsgDiv);

            // 4. Save to session and render
            const sessions = window.UI.AI.sessions || [];
            let session = sessions.find(function(s) { return s.id === window.UI.AI.currentSessionId; });
            if (!session && typeof window.UI.AI.createNewSession === 'function') {
                window.UI.AI.createNewSession('会话');
                session = sessions.find(function(s) { return s.id === window.UI.AI.currentSessionId; });
            }

            const replyText = finalReply || '(empty response)';

            if (session) {
                session.messages.push({ role: 'user', content: msg, ts: new Date().toISOString() });
                session.messages.push({ role: 'ai', content: replyText, ts: new Date().toISOString() });
                if (session.messages.length === 2) {
                    session.title = msg.length > 15 ? msg.substring(0, 15) + '...' : msg;
                }
                window.UI.AI.saveAll();
                window.UI.AI.renderMessagesByList('chatMessages', session.messages);
                window.UI.AI.renderMessagesByList('aiMainMessages', session.messages);
            } else {
                // Fallback: render inline
                var fallback = document.createElement('div');
                fallback.className = 'flex w-full mt-2 space-x-3 max-w-xs';
                fallback.innerHTML = '<div class="p-3 bg-slate-800/80 rounded-xl msg-ai leading-relaxed border border-white/5"><div class="markdown-body">' + renderMarkdown(replyText) + '</div></div>';
                chatMessages.appendChild(fallback);
            }
        } catch (err) {
            if (aiMsgDiv.parentNode) aiMsgDiv.parentNode.removeChild(aiMsgDiv);
            var sessions = window.UI.AI.sessions || [];
            var session = sessions.find(function(s) { return s.id === window.UI.AI.currentSessionId; });
            var errMsg = '服务暂时离线: ' + err.message;
            if (session) {
                session.messages.push({ role: 'user', content: msg, ts: new Date().toISOString() });
                session.messages.push({ role: 'ai', content: errMsg, ts: new Date().toISOString() });
                window.UI.AI.saveAll();
                window.UI.AI.renderMessagesByList('chatMessages', session.messages);
                window.UI.AI.renderMessagesByList('aiMainMessages', session.messages);
            } else {
                var fallback = document.createElement('div');
                fallback.className = 'flex w-full mt-2 space-x-3 max-w-xs';
                fallback.innerHTML = '<div class="p-3 bg-slate-800/80 rounded-xl msg-ai leading-relaxed border border-white/5"><div class="markdown-body">' + renderMarkdown(errMsg) + '</div></div>';
                chatMessages.appendChild(fallback);
            }
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
