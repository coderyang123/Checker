<script setup lang="ts">
import { ref, shallowRef, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { Codemirror } from 'vue-codemirror';
import { json } from '@codemirror/lang-json';
import { EditorView } from '@codemirror/view';
import { EditorState } from '@codemirror/state';
import { FileJson, Search, Database, AlertCircle, CheckCircle, LoaderCircle } from 'lucide-vue-next';

// --- Refs and State ---
const jsonContent = ref('');
const rawJsonContent = ref('');
const emptyValueResults = ref<any[]>([]);
const invalidNumericResults = ref<any[]>([]);
const errorLog = ref('');
const statusMessage = ref('Ready. Open a JSON file to begin.');
const showSqlModal = ref(false);
const sqlInput = ref('CREATE TABLE items (id INT, name TEXT, price NUMERIC, quantity INT);');
const isLoading = ref(false);
const lastAnalysisType = ref<'empty' | 'numeric' | null>(null);

const codemirrorView = shallowRef();
const extensions = [json(), EditorView.lineWrapping];

// --- Computed Properties ---
const hasJsonContent = computed(() => jsonContent.value.length > 0);
const hasEmptyValueResults = computed(() => emptyValueResults.value.length > 0);
const hasInvalidNumericResults = computed(() => invalidNumericResults.value.length > 0);

// --- Core Methods ---
async function openFile() {
  clearState();
  statusMessage.value = 'Opening file...';
  try {
    const result: string = await invoke('open_and_read_json_file');
    rawJsonContent.value = result;
    jsonContent.value = JSON.stringify(JSON.parse(result), null, 2);
    statusMessage.value = 'File loaded successfully.';
  } catch (e) {
    handleError(e);
    statusMessage.value = 'Failed to open or parse file.';
  }
}

async function findEmptyValues() {
  if (!hasJsonContent.value) return;
  setLoadingState('Finding empty values...');
  lastAnalysisType.value = 'empty';
  try {
    const result: any = await invoke('find_empty_values', { jsonStr: rawJsonContent.value });
    emptyValueResults.value = result.data;
    statusMessage.value = `Found ${result.data.length} empty values in ${result.duration_ms}ms.`;
    if (result.data.length > 0) {
      highlightResult(result.data[0].index, result.data[0].key);
    }
  } catch (e) {
    handleError(e);
  } finally {
    isLoading.value = false;
  }
}

async function findInvalidNumerics() {
  if (!hasJsonContent.value) return;
  showSqlModal.value = true;
}

async function executeInvalidNumericsCheck() {
  if (!sqlInput.value) return;
  showSqlModal.value = false;
  setLoadingState('Finding invalid numeric values...');
  lastAnalysisType.value = 'numeric';
  try {
    const result: any = await invoke('find_invalid_numeric_values', {
      jsonStr: rawJsonContent.value,
      sqlStr: sqlInput.value,
    });
    invalidNumericResults.value = result.data;
    statusMessage.value = `Found ${result.data.length} invalid numeric values in ${result.duration_ms}ms.`;
    if (result.data.length > 0) {
      highlightResult(result.data[0].index, result.data[0].key);
    }
  } catch (e) {
    handleError(e);
  } finally {
    isLoading.value = false;
  }
}

// --- UI and Editor Methods ---
function highlightResult(index: number, key: string) {
  if (!codemirrorView.value) return;

  const view = codemirrorView.value.view;
  const doc = view.state.doc;
  const searchString = `"${key}"`;
  let occurrence = 0;

  for (let i = 1; i <= doc.lines; i++) {
    const line = doc.line(i);
    let pos = line.from;
    while ((pos = line.text.indexOf(searchString, pos)) !== -1) {
      if (occurrence === index) {
        const from = line.from + pos;
        const to = from + searchString.length;
        view.dispatch({
          selection: { anchor: from, head: to },
          effects: EditorView.scrollIntoView(from, { y: 'center' }),
        });
        return;
      }
      pos += searchString.length;
      occurrence++;
    }
  }
}

// --- Helper Methods ---
function setLoadingState(message: string) {
  isLoading.value = true;
  statusMessage.value = message;
  errorLog.value = '';
  emptyValueResults.value = [];
  invalidNumericResults.value = [];
}

function clearState() {
  jsonContent.value = '';
  rawJsonContent.value = '';
  emptyValueResults.value = [];
  invalidNumericResults.value = [];
  errorLog.value = '';
  lastAnalysisType.value = null;
  statusMessage.value = 'Ready.';
}

function handleError(e: any) {
  const errorMessage = typeof e === 'string' ? e : (e instanceof Error ? e.message : 'An unknown error occurred');
  errorLog.value = errorMessage;
  statusMessage.value = 'An error occurred.';
  lastAnalysisType.value = null;
}
</script>

<template>
  <div class="app-container">
    <!-- Sidebar -->
    <aside class="sidebar">
      <header class="sidebar-header">
        <h1 class="title">JSON Checker</h1>
      </header>

      <nav class="actions">
        <button @click="openFile" :disabled="isLoading">
          <FileJson :size="16" />
          Open JSON File
        </button>
        <button @click="findEmptyValues" :disabled="!hasJsonContent || isLoading">
          <Search :size="16" />
          Find Empty Values
        </button>
        <button @click="findInvalidNumerics" :disabled="!hasJsonContent || isLoading">
          <Database :size="16" />
          Find Invalid Numerics
        </button>
      </nav>

      <div class="results-panel">
        <div v-if="isLoading" class="results-placeholder">
            <LoaderCircle :size="24" class="spinner" />
            <p>Analyzing...</p>
        </div>
        <div v-else-if="lastAnalysisType === 'empty'">
          <div v-if="hasEmptyValueResults" class="results-list">
            <h2>Empty Values Found</h2>
            <ul>
              <li v-for="item in emptyValueResults" :key="`${item.index}-${item.key}`" @click="highlightResult(item.index, item.key)">
                Object {{ item.index + 1 }}: <strong>{{ item.key }}</strong>
              </li>
            </ul>
          </div>
          <div v-else class="results-placeholder">
            <CheckCircle :size="24" class="success-icon"/>
            <p>No empty values found.</p>
          </div>
        </div>
        <div v-else-if="lastAnalysisType === 'numeric'">
          <div v-if="hasInvalidNumericResults" class="results-list">
            <h2>Invalid Numerics Found</h2>
            <ul>
              <li v-for="item in invalidNumericResults" :key="`${item.index}-${item.key}`" @click="highlightResult(item.index, item.key)">
                Object {{ item.index + 1 }}: <strong>{{ item.key }}</strong> (value: "{{ item.value }}")
              </li>
            </ul>
          </div>
           <div v-else class="results-placeholder">
            <CheckCircle :size="24" class="success-icon"/>
            <p>No invalid numeric values found.</p>
          </div>
        </div>
         <div v-else-if="errorLog" class="results-placeholder">
            <AlertCircle :size="24" class="error-icon"/>
            <p>An error occurred during analysis.</p>
        </div>
      </div>

      <footer class="status-footer">
        <p class="status-message" :class="{ 'error': errorLog }">{{ statusMessage }}</p>
        <div v-if="errorLog" class="error-log">
          <pre>{{ errorLog }}</pre>
        </div>
      </footer>
    </aside>

    <!-- Main Content -->
    <main class="main-content">
      <Codemirror
        v-model="jsonContent"
        placeholder="Open a JSON file to see its content here..."
        :style="{ height: '100%' }"
        :autofocus="true"
        :indent-with-tab="true"
        :tab-size="2"
        :extensions="extensions"
        @ready="codemirrorView = $event"
        :disabled="isLoading"
      />
    </main>

    <!-- SQL Modal -->
    <div v-if="showSqlModal" class="modal-overlay" @click.self="showSqlModal = false">
      <div class="modal-content">
        <h2>Enter SQL Schema</h2>
        <p>Provide the `CREATE TABLE` SQL statement to identify numeric fields.</p>
        <textarea v-model="sqlInput" rows="10"></textarea>
        <div class="modal-actions">
          <button @click="showSqlModal = false" class="secondary">Cancel</button>
          <button @click="executeInvalidNumericsCheck" class="primary">Analyze</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style>
/* --- CSS Variables for Theming --- */
:root {
  --font-sans: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif;
  
  /* Light Mode */
  --color-bg-light: #f2f2f7;
  --color-bg-sidebar-light: #e9e9ed;
  --color-bg-main-light: #ffffff;
  --color-border-light: #d1d1d6;
  --color-text-light: #1d1d1f;
  --color-text-muted-light: #6e6e73;
  --color-primary-light: #007aff;
  --color-primary-hover-light: #0071e3;
  --color-error-light: #ff3b30;
  --color-success-light: #34c759;
  --color-button-bg-light: rgba(120, 120, 128, 0.16);
  --color-button-hover-light: rgba(120, 120, 128, 0.24);

  /* Dark Mode */
  --color-bg-dark: #1c1c1e;
  --color-bg-sidebar-dark: #2c2c2e;
  --color-bg-main-dark: #1c1c1e;
  --color-border-dark: #3a3a3c;
  --color-text-dark: #f5f5f7;
  --color-text-muted-dark: #8e8e93;
  --color-primary-dark: #0a84ff;
  --color-primary-hover-dark: #3395ff;
  --color-error-dark: #ff453a;
  --color-success-dark: #32d74b;
  --color-button-bg-dark: rgba(120, 120, 128, 0.24);
  --color-button-hover-dark: rgba(120, 120, 128, 0.32);
}

/* --- Apply Theme --- */
@media (prefers-color-scheme: light) {
  :root {
    --color-bg: var(--color-bg-light);
    --color-bg-sidebar: var(--color-bg-sidebar-light);
    --color-bg-main: var(--color-bg-main-light);
    --color-border: var(--color-border-light);
    --color-text: var(--color-text-light);
    --color-text-muted: var(--color-text-muted-light);
    --color-primary: var(--color-primary-light);
    --color-primary-hover: var(--color-primary-hover-light);
    --color-error: var(--color-error-light);
    --color-success: var(--color-success-light);
    --color-button-bg: var(--color-button-bg-light);
    --color-button-hover: var(--color-button-hover-light);
  }
}

@media (prefers-color-scheme: dark) {
  :root {
    --color-bg: var(--color-bg-dark);
    --color-bg-sidebar: var(--color-bg-sidebar-dark);
    --color-bg-main: var(--color-bg-main-dark);
    --color-border: var(--color-border-dark);
    --color-text: var(--color-text-dark);
    --color-text-muted: var(--color-text-muted-dark);
    --color-primary: var(--color-primary-dark);
    --color-primary-hover: var(--color-primary-hover-dark);
    --color-error: var(--color-error-dark);
    --color-success: var(--color-success-dark);
    --color-button-bg: var(--color-button-bg-dark);
    --color-button-hover: var(--color-button-hover-dark);
  }
}

/* --- Global & Layout --- */
html, body {
  margin: 0;
  padding: 0;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
  font-family: var(--font-sans);
  background-color: var(--color-bg);
  color: var(--color-text);
  font-size: 14px;
}

.app-container {
  display: flex;
  height: 100vh;
}

.sidebar {
  width: 300px;
  flex-shrink: 0;
  background-color: var(--color-bg-sidebar);
  display: flex;
  flex-direction: column;
  padding: 16px;
  box-sizing: border-box;
  border-right: 1px solid var(--color-border);
}

.main-content {
  flex-grow: 1;
  height: 100vh;
  overflow: hidden;
  background-color: var(--color-bg-main);
}

/* --- Sidebar Components --- */
.sidebar-header {
  padding: 8px 8px 24px 8px;
}
.title {
  font-size: 22px;
  margin: 0;
  font-weight: 600;
}

.actions {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

button {
  display: flex;
  align-items: center;
  gap: 10px;
  background-color: transparent;
  color: var(--color-text);
  border: none;
  border-radius: 8px;
  padding: 10px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: background-color 0.2s;
  text-align: left;
}
button:hover {
  background-color: var(--color-button-hover);
}
button:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.results-panel {
  flex-grow: 1;
  overflow-y: auto;
  margin-top: 20px;
  padding-top: 20px;
  border-top: 1px solid var(--color-border);
}

.results-placeholder {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    text-align: center;
    color: var(--color-text-muted);
}
.results-placeholder p {
    margin-top: 12px;
    font-size: 14px;
}
.spinner {
    animation: spin 1.5s linear infinite;
}
@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}
.success-icon { color: var(--color-success); }
.error-icon { color: var(--color-error); }


.results-list h2 {
  font-size: 16px;
  font-weight: 600;
  color: var(--color-text);
  margin: 0 0 12px 0;
  padding: 0 8px;
}
.results-list ul {
  list-style: none;
  padding: 0;
  margin: 0;
}
.results-list li {
  padding: 10px;
  border-radius: 8px;
  cursor: pointer;
  transition: background-color 0.2s;
  font-size: 13px;
}
.results-list li:hover {
  background-color: var(--color-button-hover);
}
.results-list li strong {
  font-weight: 600;
  color: var(--color-primary);
}

/* --- Footer & Status --- */
.status-footer {
  padding-top: 16px;
  border-top: 1px solid var(--color-border);
  font-size: 12px;
  color: var(--color-text-muted);
  flex-shrink: 0;
}
.status-message {
  margin: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.status-message.error {
  color: var(--color-error);
}
.error-log {
  margin-top: 8px;
  padding: 8px;
  background-color: rgba(255, 59, 48, 0.1);
  border: 1px solid var(--color-error);
  border-radius: 4px;
  color: var(--color-error);
}
.error-log pre {
  white-space: pre-wrap;
  word-break: break-all;
  margin: 0;
  font-size: 11px;
}

/* --- CodeMirror --- */
.cm-editor {
  height: 100%;
  font-size: 14px;
  background: transparent;
}
.cm-gutters {
  background-color: var(--color-bg-main) !important;
  border-right: 1px solid var(--color-border);
}
.cm-content {
  caret-color: var(--color-primary);
}
.cm-selectionBackground {
  background: rgba(10, 132, 255, 0.2) !important;
}

/* --- Modal --- */
.modal-overlay {
  position: fixed;
  inset: 0;
  background-color: rgba(0, 0, 0, 0.5);
  backdrop-filter: blur(4px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}
.modal-content {
  background-color: var(--color-bg-sidebar);
  padding: 24px;
  border-radius: 12px;
  border: 1px solid var(--color-border);
  width: 500px;
  box-shadow: 0 10px 30px rgba(0,0,0,0.2);
}
.modal-content h2 {
  margin-top: 0;
  font-size: 18px;
  font-weight: 600;
}
.modal-content p {
  color: var(--color-text-muted);
  margin-bottom: 16px;
  font-size: 13px;
}
.modal-content textarea {
  width: 100%;
  box-sizing: border-box;
  background-color: var(--color-bg);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  color: var(--color-text);
  padding: 10px;
  font-family: var(--font-sans);
  margin-bottom: 20px;
  resize: vertical;
}
.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
}
.modal-actions button {
  padding: 8px 16px;
}
.modal-actions button.primary {
  background-color: var(--color-primary);
  color: white;
}
.modal-actions button.primary:hover {
  background-color: var(--color-primary-hover);
}
.modal-actions button.secondary {
  background-color: var(--color-button-bg);
}
.modal-actions button.secondary:hover {
  background-color: var(--color-button-hover);
}
</style>