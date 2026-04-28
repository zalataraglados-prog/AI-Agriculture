async function doLogin() {
  const username = document.getElementById('username').value.trim();
  const password = document.getElementById('password').value.trim();
  const deviceId = document.getElementById('deviceId').value.trim();
  const msg = document.getElementById('msg');
  if (!username || !password) {
    msg.textContent = '璇疯緭鍏ヨ处鍙峰拰瀵嗙爜';
    return;
  }
  msg.textContent = '姝ｅ湪鐧诲綍...';
  try {
    const res = await fetch('/api/login', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username, password })
    });
    const data = await res.json().catch(() => ({}));
    if (data && data.success) {
      if (deviceId) localStorage.setItem('device_id', deviceId);
      const q = deviceId ? `?device_id=${encodeURIComponent(deviceId)}` : '';
      location.href = `index.html${q}`;
      return;
    }
    msg.textContent = data.message || '鐧诲綍澶辫触';
  } catch (e) {
    msg.textContent = `鐧诲綍澶辫触: ${e.message}`;
  }
}

document.getElementById('loginBtn').addEventListener('click', doLogin);
document.getElementById('password').addEventListener('keydown', (e) => {
  if (e.key === 'Enter') doLogin();
});
