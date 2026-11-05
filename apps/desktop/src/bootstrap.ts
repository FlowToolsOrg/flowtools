import './csp-schema-policy'

void import('./main').catch(() => {
  const root = document.getElementById('root')
  if (root) root.textContent = '应用加载失败，请关闭窗口后重新打开。'
})
