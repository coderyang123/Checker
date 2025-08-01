<script setup lang="ts">
import {computed, ref, shallowRef} from 'vue';
import {invoke} from '@tauri-apps/api/core';
import {Codemirror} from 'vue-codemirror';
import {json} from '@codemirror/lang-json';
import {EditorView} from '@codemirror/view';

// --- Refs and State ---
const jsonContent = ref('');
const rawJsonContent = ref(''); // Store the original, unformatted content
const emptyValueResults = ref<any[]>([]);
const invalidNumericResults = ref<any[]>([]);
const errorLog = ref('');
const statusMessage = ref('Ready. Open a JSON file to begin.');
const showSqlModal = ref(false);
const sqlInput = ref('CREATE TABLE items (id INT, name TEXT, price NUMERIC, quantity INT);');
const isLoading = ref(false);

const codemirrorView = shallowRef();
const extensions = [json(), EditorView.lineWrapping];

// --- Computed Properties ---
const hasJsonContent = computed(() => jsonContent.value.length > 0);
const hasEmptyValueResults = computed(() => emptyValueResults.value.length > 0);
const hasInvalidNumericResults = computed(() => invalidNumericResults.value.length > 0);

// --- Core Methods ---
async function openFile() {
  clearState();
  statusMessage.value = 'Opening file dialog...';
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
  try {
    const result: any = await invoke('find_empty_values', {jsonStr: rawJsonContent.value});
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
          selection: {anchor: from, head: to},
          effects: EditorView.scrollIntoView(from, {y: 'center'}),
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
  statusMessage.value = 'Ready.';
}

function handleError(e: any) {
  const errorMessage = typeof e === 'string' ? e : (e instanceof Error ? e.message : 'An unknown error occurred');
  errorLog.value = errorMessage;
  statusMessage.value = 'An error occurred. See details below.';
}
</script>

<template>
  <div class="app-container">
    <!-- Sidebar -->
    <aside class="sidebar">
      <header class="sidebar-header">
        <h1 class="title">JSON Checker</h1>
        <p class="subtitle">Tauri + Vue + Rust</p>
      </header>

      <nav class="actions">
        <button @click="openFile" :disabled="isLoading">Open JSON File</button>
        <button @click="findEmptyValues" :disabled="!hasJsonContent || isLoading">Find Empty Values</button>
        <button @click="findInvalidNumerics" :disabled="!hasJsonContent || isLoading">Find Invalid Numerics</button>
      </nav>

      <div class="results-panel">
        <div v-if="isLoading" class="loader"></div>

        <div v-if="hasEmptyValueResults" class="results-list">
          <h2>Empty Values Found</h2>
          <ul>
            <li v-for="item in emptyValueResults" :key="`${item.index}-${item.key}`"
                @click="highlightResult(item.index, item.key)">
              Object {{ item.index + 1 }}: key <strong>{{ item.key }}</strong>
            </li>
          </ul>
        </div>

        <div v-if="hasInvalidNumericResults" class="results-list">
          <h2>Invalid Numerics Found</h2>
          <ul>
            <li v-for="item in invalidNumericResults" :key="`${item.index}-${item.key}`"
                @click="highlightResult(item.index, item.key)">
              Object {{ item.index + 1 }}: key <strong>{{ item.key }}</strong> (value: "{{ item.value }}")
            </li>
          </ul>
        </div>
      </div>

      <footer class="status-footer">
        <p class="status-message">{{ statusMessage }}</p>
        <div v-if="errorLog" class="error-log">
          <strong>Error Details:</strong>
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
          <button @click="showSqlModal = false">Cancel</button>
          <button @click="executeInvalidNumericsCheck" class="primary">Analyze</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style>
/* --- Global & Layout --- */
:root {
  --color-bg: #2a2a2e;
  --color-bg-sidebar: #1e1e20;
  --color-bg-main: #2d2d30;
  --color-border: #4a4a50;
  --color-text: #e0e0e0;
  --color-text-muted: #9e9e9e;
  --color-primary: #42b883;
  --color-primary-hover: #53c894;
  --color-error: #e57373;
  --font-sans: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif;
}

html, body {
  margin: 0;
  padding: 0;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
  font-family: var(--font-sans);
  background-color: var(--color-bg);
  color: var(--color-text);
}

.app-container {
  display: flex;
  height: 100vh;
}

.sidebar {
  width: 320px;
  background-color: var(--color-bg-sidebar);
  border-right: 1px solid var(--color-border);
  display: flex;
  flex-direction: column;
  padding: 20px;
  box-sizing: border-box;
}

.main-content {
  flex-grow: 1;
  height: 100vh;
  overflow: hidden;
  background-color: var(--color-bg-main);
}

/* --- Sidebar Components --- */
.sidebar-header {
  padding-bottom: 20px;
  border-bottom: 1px solid var(--color-border);
}

.title {
  font-size: 24px;
  margin: 0;
  color: white;
}

.subtitle {
  font-size: 14px;
  margin: 4px 0 0;
  color: var(--color-text-muted);
}

.actions {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 20px 0;
}

button {
  background-color: #3a3a3d;
  color: var(--color-text);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  padding: 10px 15px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: background-color 0.2s, border-color 0.2s;
  text-align: left;
}

button:hover {
  background-color: #4a4a4d;
}

button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

button.primary {
  background-color: var(--color-primary);
  border-color: var(--color-primary);
  color: white;
}

button.primary:hover {
  background-color: var(--color-primary-hover);
  border-color: var(--color-primary-hover);
}

.results-panel {
  flex-grow: 1;
  overflow-y: auto;
  padding-top: 10px;
}

.results-list h2 {
  font-size: 16px;
  color: var(--color-primary);
  margin-top: 0;
  margin-bottom: 8px;
}

.results-list ul {
  list-style: none;
  padding: 0;
  margin: 0;
}

.results-list li {
  padding: 8px;
  border-radius: 4px;
  cursor: pointer;
  transition: background-color 0.2s;
  font-size: 13px;
}

.results-list li:hover {
  background-color: #3a3a3d;
}

.results-list li strong {
  color: var(--color-primary);
  font-weight: 600;
}

/* --- Footer & Status --- */
.status-footer {
  padding-top: 15px;
  border-top: 1px solid var(--color-border);
  font-size: 12px;
  color: var(--color-text-muted);
}

.status-message {
  margin: 0;
}

.error-log {
  margin-top: 10px;
  padding: 8px;
  background-color: rgba(229, 115, 115, 0.1);
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
}

.cm-gutters {
  background-color: var(--color-bg-sidebar) !important;
  border-right: 1px solid var(--color-border);
}

.cm-content {
  caret-color: var(--color-primary);
}

.cm-selectionBackground {
  background: rgba(66, 184, 131, 0.2) !important;
}

/* --- Modal --- */
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}

.modal-content {
  background-color: var(--color-bg-sidebar);
  padding: 24px;
  border-radius: 8px;
  border: 1px solid var(--color-border);
  width: 500px;
}

.modal-content h2 {
  margin-top: 0;
}

.modal-content p {
  color: var(--color-text-muted);
  margin-bottom: 16px;
}

.modal-content textarea {
  width: 100%;
  box-sizing: border-box;
  background-color: var(--color-bg-main);
  border: 1px solid var(--color-border);
  border-radius: 4px;
  color: var(--color-text);
  padding: 8px;
  font-family: var(--font-sans);
  margin-bottom: 16px;
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
}

/* --- Loader --- */
.loader {
  border: 4px solid #f3f3f3;
  border-top: 4px solid var(--color-primary);
  border-radius: 50%;
  width: 30px;
  height: 30px;
  animation: spin 1s linear infinite;
  margin: 20px auto;
}

@keyframes spin {
  0% {
    transform: rotate(0deg);
  }
  100% {
    transform: rotate(360deg);
  }
}
</style>
