/**
 * Chat & Agent — SSE streaming, multi-turn, session-aware.
 * Sends POST /api/v1/chat/stream (SSE) with /api/v1/chat fallback.
 * Uses persistent session_id for multi-turn conversation on server side.
 *
 * JSON contract:
 *   Input:  {message, context, session_id}
 *   Output: {reply}
 */
window.CHAT = (() => {
    const SESSION_ID_KEY = 'agri_session_id';

    function getSessionId() {
        let sid = localStorage.getItem(SESSION_ID_KEY);
        if (!sid) {
            sid = 'agri-' + Date.now() + '-' + Math.random().toString(36).slice(2, 8);
            localStorage.setItem(SESSION_ID_KEY, sid);
        }
        return sid;
    }

    function escapeHtml(text) {
        return (text || '')
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;')
            .replace(/'/g, '&#39;');
    }

    function renderInline(text) {
        let out = escapeHtml(text);
        out = out.replace(/`([^`]+)`/g, '<code class="md-inline-code">$1</code>');
        out = out.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');
        out = out.replace(/\*([^*]+)\*/g, '<em>$1</em>');
        return out;
    }

    function renderMarkdown(raw) {
        const source = (raw || '');
        const parts = source.split(/```/);
        return parts.map((part, idx) => {
            if (idx % 2 === 1) {
                return `<pre class="md-pre"><code>${escapeHtml(part.trim())}</code></pre>`;
            }
            const lines = part.split(/\r?\n/);
            let inList = false;
            const buf = [];
            lines.forEach(line => {
                const bullet = line.match(/^\s*[-*]\s+(.+)$/);
                if (bullet) {
                    if (!inList) { inList = true; buf.push('<ul class="md-list">'); }
                    buf.push(`<li>${renderInline(bullet[1])}</li>`);
                    return;
                }
                if (inList) { buf.push('</ul>'); inList = false; }
                if (!line.trim()) { buf.push('<br>'); return; }
                const h = line.match(/^\s{0,3}#{1,6}\s+(.+)$/);
                if (h) { buf.push(`<p class="md-heading">${renderInline(h[1])}</p>`); return; }
                buf.push(`<p>${renderInline(line)}</p>`);
            });
            if (inList) buf.push('</ul>');
            return buf.join('');
        }).join('') || '<p>-</p>';
    }

    // ── Gather sensor context for chat ─────────────────────────────────────

    function getSensorContext() {
        const ctx = {};
        const telemetry = window.API?.getTelemetry?.() || [];
        const latest = telemetry[telemetry.length - 1];
        if (latest?.fields) {
            if (latest.fields.vwc !== undefined) ctx.current_vwc = latest.fields.vwc;
            if (latest.fields.temperature !== undefined) ctx.temperature = latest.fields.temperature;
            if (latest.fields.humidity !== undefined) ctx.humidity = latest.fields.humidity;
            if (latest.fields.ec !== undefined) ctx.ec = latest.fields.ec;
            if (latest.fields.ph !== undefined) ctx.ph = latest.fields.ph;
        }
        const crop = window.UI?.HomePositioning?.selectedCropType;
        if (crop) ctx.crop_type = crop;
        return ctx;
    }

    // ── SSE Streaming request ──────────────────────────────────────────────

    async function sendStreaming(msg, context, onChunk, onDone, onError) {
        try {
            const resp = await fetch('/api/v1/chat/stream', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    message: msg,
                    context: context || {},
                    session_id: getSessionId(),
                }),
            });
            if (!resp.ok) {
                let errMsg = `请求失败 (${resp.status})`;
                try { const d = await resp.json(); errMsg = d?.message || errMsg; } catch (_) {}
                onError(errMsg);
                return;
            }
            const reader = resp.body.getReader();
            const decoder = new TextDecoder();
            let buf = '';
            while (true) {
                const { done, value } = await reader.read();
                if (done) { onDone(); return; }
                buf += decoder.decode(value, { stream: true });
                const lines = buf.split('\n');
                buf = lines.pop() || '';
                for (const line of lines) {
                    const t = line.trim();
                    if (!t.startsWith('data: ')) continue;
                    try {
                        const data = JSON.parse(t.slice(6));
                        if (data.done) { onDone(); return; }
                        if (data.error) { onError(data.error); return; }
                        if (data.text) onChunk(data.text);
                    } catch (_) {}
                }
            }
        } catch (err) {
            onError(err.message);
        }
    }

    // ── Non-streaming fallback ─────────────────────────────────────────────

    async function sendNonStreaming(msg, context) {
        const resp = await fetch('/api/v1/chat', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                message: msg,
                context: context || {},
                session_id: getSessionId(),
            }),
        });
        let data;
        try { data = await resp.json(); } catch (_) {}
        if (!resp.ok) throw new Error(data?.message || `请求失败 (${resp.status})`);
        if (!data?.reply) throw new Error('回复为空');
        return data.reply;
    }

    // ── Loading indicator helper ───────────────────────────────────────────

    function showLoader(remove) {
        ['chatMessages', 'aiMainMessages'].forEach(id => {
            const container = document.getElementById(id);
            if (!container) return;
            // Remove existing loader
            const existing = document.getElementById('loader-' + id);
            if (existing) existing.remove();
            if (remove) return;
            // Create loading bubble (3 bouncing dots, matches original style)
            const loader = document.createElement('div');
            loader.id = 'loader-' + id;
            loader.className = 'flex w-full mt-2 space-x-3 max-w-xs';
            loader.innerHTML = [
                '<div class="p-3 bg-slate-800/80 rounded-xl msg-ai flex items-center gap-2 border border-white/5">',
                '<div class="w-1.5 h-1.5 bg-emerald-400 rounded-full animate-bounce"></div>',
                '<div class="w-1.5 h-1.5 bg-emerald-400 rounded-full animate-bounce" style="animation-delay: 0.1s"></div>',
                '<div class="w-1.5 h-1.5 bg-emerald-400 rounded-full animate-bounce" style="animation-delay: 0.2s"></div>',
                '</div>',
            ].join('');
            container.appendChild(loader);
            container.scrollTop = container.scrollHeight;
        });
    }

    function removeLoader() {
        showLoader(true);
    }

    // ── Submit: unified handler for chatInput + aiMainInput ────────────────

    async function handleSubmit(e) {
        if (e) {
            if (typeof e.preventDefault === 'function') e.preventDefault();
            if (e.msg) {
                const mainInput = document.getElementById('aiMainInput');
                mainInput.value = e.msg;
            }
        }

        const chatInput = document.getElementById('chatInput');
        const mainInput = document.getElementById('aiMainInput');

        if (window.UI?.AI?.isTyping) return;

        const input = (chatInput && chatInput.value.trim())
            ? chatInput
            : (mainInput && mainInput.value.trim())
                ? mainInput
                : null;
        if (!input) return;

        const msg = input.value.trim();
        input.value = '';
        if (input === chatInput) input.focus();

        // Add user message to session
        if (window.UI?.AI) {
            window.UI.AI.addMessage?.('user', msg);
        }

        window.UI.AI.isTyping = true;

        const instructions = window.UI?.AI?.instructionList || [];
        const fullMsg = instructions.length
            ? instructions.join('\n') + '\n\n' + msg
            : msg;

        const context = getSensorContext();

        // Show loading dots
        showLoader(false);

        let fullText = '';
        let bubbles = {};

        function getContainer(id) {
            return document.getElementById(id);
        }

        function createBubble(container) {
            const div = document.createElement('div');
            div.className = 'flex w-full mt-2 space-x-3 max-w-xs';
            div.innerHTML = '<div class="p-3 bg-slate-800/80 rounded-xl msg-ai leading-relaxed border border-white/5 markdown-body stream-bubble"><span class="stream-cursor"></span></div>';
            container.appendChild(div);
            container.scrollTop = container.scrollHeight;
            return div;
        }

        sendStreaming(
            fullMsg,
            context,
            // onChunk — first chunk removes loader and shows bubble
            chunk => {
                fullText += chunk;
                ['chatMessages', 'aiMainMessages'].forEach(id => {
                    const container = getContainer(id);
                    if (!container) return;
                    if (!bubbles[id]) {
                        bubbles[id] = createBubble(container);
                    }
                    const body = bubbles[id].querySelector('.markdown-body');
                    body.innerHTML = renderMarkdown(fullText);
                    let cursor = body.querySelector('.stream-cursor');
                    if (!cursor) {
                        cursor = document.createElement('span');
                        cursor.className = 'stream-cursor';
                    }
                    body.appendChild(cursor);
                    container.scrollTop = container.scrollHeight;
                });
                // Remove loader on first chunk
                if (fullText.length <= chunk.length) {
                    removeLoader();
                }
            },
            // onDone
            () => {
                removeLoader();
                Object.values(bubbles).forEach(b => {
                    if (!b) return;
                    const c = b.querySelector('.stream-cursor');
                    if (c) c.remove();
                });
                if (window.UI?.AI) {
                    window.UI.AI.addMessage?.('ai', fullText);
                }
                window.UI.AI.isTyping = false;
            },
            // onError — fallback to non-streaming
            async errMsg => {
                removeLoader();
                Object.values(bubbles).forEach(b => { if (b) b.remove(); });
                bubbles = {};
                window.UI.AI.isTyping = true;
                try {
                    const reply = await sendNonStreaming(fullMsg, context);
                    if (window.UI?.AI) {
                        window.UI.AI.addMessage?.('ai', reply);
                    }
                } catch (fallbackErr) {
                    if (window.UI?.AI) {
                        window.UI.AI.addMessage?.('ai', `服务暂时离线: ${fallbackErr.message}`);
                    }
                } finally {
                    window.UI.AI.isTyping = false;
                }
            }
        );
    }

    return {
        handleSubmit,
        sendNonStreaming,
        renderMarkdown,
        escapeHtml,
        getSessionId,
    };
})();
