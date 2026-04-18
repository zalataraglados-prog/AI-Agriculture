async function doLogin() {
  const username = document.getElementById('username').value.trim();
  const password = document.getElementById('password').value.trim();
  const deviceId = document.getElementById('deviceId').value.trim();
  const msg = document.getElementById('msg');
  
  if (!username || !password) {
    msg.textContent = '请输入账号和密码以进入中枢';
    return;
  }
  
  msg.textContent = '权限验证通过，正在进入系统...';
  
  // Official behavior: Store device_id if provided and redirect immediately
  if (deviceId) {
    localStorage.setItem('device_id', deviceId);
  } else {
    localStorage.removeItem('device_id');
  }
  
  const q = deviceId ? `?device_id=${encodeURIComponent(deviceId)}` : '';
  setTimeout(() => {
    location.href = `index.html${q}`;
  }, 800);
}

document.getElementById('loginBtn').addEventListener('click', doLogin);
document.getElementById('password').addEventListener('keydown', (e) => {
  if (e.key === 'Enter') doLogin();
});

