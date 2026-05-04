const DB_NAME = 'flowtools-plugins'
const DB_VERSION = 1
const STORE_NAME = 'external-plugins'

export interface StoredPluginFile {
  id: string
  name: string
  fileName: string
  type: string
  data: ArrayBuffer
}

function openDB(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION)

    req.onupgradeneeded = () => {
      const db = req.result
      if (!db.objectStoreNames.contains(STORE_NAME)) {
        db.createObjectStore(STORE_NAME, { keyPath: 'id' })
      }
    }
    req.onsuccess = () => resolve(req.result)
    req.onerror = () => reject(req.error)
  })
}

export async function savePluginFile(entry: StoredPluginFile): Promise<void> {
  const db = await openDB()
  const tx = db.transaction(STORE_NAME, 'readwrite')
  tx.objectStore(STORE_NAME).put(entry)
  await new Promise<void>((resolve, reject) => {
    tx.oncomplete = () => resolve()
    tx.onerror = () => reject(tx.error)
  })
  db.close()
}

export async function loadAllPluginFiles(): Promise<StoredPluginFile[]> {
  const db = await openDB()
  const tx = db.transaction(STORE_NAME, 'readonly')
  const req = tx.objectStore(STORE_NAME).getAll()
  const result = await new Promise<StoredPluginFile[]>((resolve, reject) => {
    req.onsuccess = () => resolve(req.result as StoredPluginFile[])
    req.onerror = () => reject(req.error)
  })
  db.close()
  return result
}

export async function removePluginFile(id: string): Promise<void> {
  const db = await openDB()
  const tx = db.transaction(STORE_NAME, 'readwrite')
  tx.objectStore(STORE_NAME).delete(id)
  await new Promise<void>((resolve, reject) => {
    tx.oncomplete = () => resolve()
    tx.onerror = () => reject(tx.error)
  })
  db.close()
}
