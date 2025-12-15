function byId(id) {
  return document.getElementById(id);
}

function setText(el, text) {
  if (!el) return;
  el.textContent = text;
}

function setPill(el, text, kind) {
  if (!el) return;
  el.classList.remove('pill--good', 'pill--bad', 'pill--neutral');
  if (kind === 'good') el.classList.add('pill--good');
  else if (kind === 'bad') el.classList.add('pill--bad');
  else el.classList.add('pill--neutral');
  el.textContent = text;
}

function fmtBytes(bytes) {
  if (bytes == null || Number.isNaN(bytes)) return '-';
  const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB'];
  let v = Number(bytes);
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  const digits = i === 0 ? 0 : 2;
  return `${v.toFixed(digits)} ${units[i]}`;
}

function fmtNumber(n) {
  if (n == null || Number.isNaN(n)) return '-';
  return String(n);
}

function fmtTime(tsMs) {
  if (!tsMs) return '';
  try {
    const d = new Date(tsMs);
    return `Last update: ${d.toLocaleTimeString()}`;
  } catch {
    return '';
  }
}

function jsonFetch(url, options) {
  return fetch(url, {
    headers: {
      'Content-Type': 'application/json',
    },
    ...options,
  }).then(async (res) => {
    const text = await res.text();
    let payload;
    try {
      payload = text ? JSON.parse(text) : null;
    } catch {
      payload = text;
    }

    if (!res.ok) {
      const msg = payload && payload.error ? payload.error : `HTTP ${res.status}`;
      throw new Error(msg);
    }
    return payload;
  });
}

const ui = {
  summary: null,
  editing: {
    cpu: false,
    memory: false,
    network: false,
    disk: false,
    chaos: false,
  },
  forms: {},
};

function buildMetricCards() {
  const container = byId('metricsCards');
  if (!container) return;

  const cards = [
    {
      key: 'cpu',
      title: 'CPU',
      rows: [
        { label: 'Target', id: 'm_cpu_target', render: (s) => `${fmtNumber(s.metrics.cpu_target_millicores)} m` },
        { label: 'Threads', id: 'm_cpu_threads', render: (s) => fmtNumber(s.metrics.cpu_active_threads) },
        { label: 'Cycles', id: 'm_cpu_cycles', render: (s) => fmtNumber(s.metrics.cpu_cycle_count) },
      ],
    },
    {
      key: 'memory',
      title: 'Memory',
      rows: [
        { label: 'Target', id: 'm_mem_target', render: (s) => fmtBytes(s.metrics.memory_target_bytes) },
        { label: 'Allocated', id: 'm_mem_alloc', render: (s) => fmtBytes(s.metrics.memory_allocated_bytes) },
        { label: 'Cycles', id: 'm_mem_cycles', render: (s) => fmtNumber(s.metrics.memory_cycle_count) },
      ],
    },
    {
      key: 'network',
      title: 'Network',
      rows: [
        { label: 'Target', id: 'm_net_target', render: (s) => fmtNumber(s.configs.network.connections) },
        { label: 'Active', id: 'm_net_active', render: (s) => fmtNumber(s.metrics.network_active_connections) },
        { label: 'Requests', id: 'm_net_req', render: (s) => fmtNumber(s.metrics.network_requests_total) },
        { label: 'Errors', id: 'm_net_err', render: (s) => fmtNumber(s.metrics.network_errors_total) },
        { label: 'Cycles', id: 'm_net_cycles', render: (s) => fmtNumber(s.metrics.network_cycle_count) },
      ],
    },
    {
      key: 'disk',
      title: 'Disk',
      rows: [
        { label: 'Target', id: 'm_disk_target', render: (s) => `${fmtNumber(s.metrics.disk_target_mbps)} MB/s` },
        { label: 'Actual', id: 'm_disk_actual', render: (s) => `${fmtNumber(s.metrics.disk_actual_mbps)} MB/s` },
        { label: 'Wrote', id: 'm_disk_w', render: (s) => fmtBytes(s.metrics.disk_bytes_written_total) },
        { label: 'Read', id: 'm_disk_r', render: (s) => fmtBytes(s.metrics.disk_bytes_read_total) },
        { label: 'Errors', id: 'm_disk_err', render: (s) => fmtNumber(s.metrics.disk_io_errors_total) },
        { label: 'Cycles', id: 'm_disk_cycles', render: (s) => fmtNumber(s.metrics.disk_cycle_count) },
      ],
    },
  ];

  container.innerHTML = '';

  for (const c of cards) {
    const card = document.createElement('div');
    card.className = 'card';

    const header = document.createElement('div');
    header.className = 'card__header';

    const title = document.createElement('h3');
    title.className = 'card__title';
    title.textContent = c.title;
    header.appendChild(title);

    card.appendChild(header);

    const kv = document.createElement('div');
    kv.className = 'kv';

    for (const r of c.rows) {
      const row = document.createElement('div');
      row.className = 'kv__row';

      const k = document.createElement('div');
      k.className = 'kv__key';
      k.textContent = r.label;

      const v = document.createElement('div');
      v.className = 'kv__value';
      v.id = r.id;
      v.textContent = '-';

      row.appendChild(k);
      row.appendChild(v);
      kv.appendChild(row);
    }

    card.appendChild(kv);
    container.appendChild(card);
  }
}

function createField(def, initialValue) {
  const wrap = document.createElement('div');
  wrap.className = 'field';

  const label = document.createElement('div');
  label.className = 'field__label';
  label.textContent = def.label;

  let input;
  if (def.type === 'select') {
    input = document.createElement('select');
    input.className = 'input';
    for (const opt of def.options) {
      const o = document.createElement('option');
      o.value = opt.value;
      o.textContent = opt.label;
      input.appendChild(o);
    }
    input.value = initialValue ?? def.options[0]?.value;
  } else if (def.type === 'checkbox') {
    input = document.createElement('input');
    input.type = 'checkbox';
    input.checked = Boolean(initialValue);
  } else if (def.type === 'number') {
    input = document.createElement('input');
    input.type = 'number';
    input.className = 'input';
    if (def.min != null) input.min = String(def.min);
    if (def.max != null) input.max = String(def.max);
    if (def.step != null) input.step = String(def.step);
    input.value = initialValue != null ? String(initialValue) : '';
  } else {
    input = document.createElement('input');
    input.type = 'text';
    input.className = 'input';
    input.placeholder = def.placeholder || '';
    input.value = initialValue != null ? String(initialValue) : '';
  }

  wrap.appendChild(label);

  if (def.type === 'checkbox') {
    const inline = document.createElement('div');
    inline.className = 'inline';
    inline.appendChild(input);
    const text = document.createElement('div');
    text.className = 'muted';
    text.textContent = def.hint || '';
    inline.appendChild(text);
    wrap.appendChild(inline);
  } else {
    wrap.appendChild(input);
    if (def.hint) {
      const hint = document.createElement('div');
      hint.className = 'field__hint';
      hint.textContent = def.hint;
      wrap.appendChild(hint);
    }
  }

  return { wrap, input };
}

function createStringList(def, values) {
  const wrap = document.createElement('div');
  wrap.className = 'field';

  const label = document.createElement('div');
  label.className = 'field__label';
  label.textContent = def.label;

  const list = document.createElement('div');
  list.className = 'list';

  const addBtn = document.createElement('button');
  addBtn.type = 'button';
  addBtn.className = 'smallBtn';
  addBtn.textContent = 'Add';

  function addRow(v) {
    const row = document.createElement('div');
    row.className = 'listRow';

    const input = document.createElement('input');
    input.type = 'text';
    input.className = 'input';
    input.placeholder = def.placeholder || '';
    input.value = v || '';

    const rm = document.createElement('button');
    rm.type = 'button';
    rm.className = 'smallBtn';
    rm.textContent = 'Remove';
    rm.addEventListener('click', () => row.remove());

    row.appendChild(input);
    row.appendChild(rm);
    list.appendChild(row);
  }

  addBtn.addEventListener('click', () => addRow(''));

  wrap.appendChild(label);
  wrap.appendChild(list);

  const actions = document.createElement('div');
  actions.className = 'inline';
  actions.appendChild(addBtn);
  if (def.hint) {
    const hint = document.createElement('div');
    hint.className = 'muted';
    hint.textContent = def.hint;
    actions.appendChild(hint);
  }
  wrap.appendChild(actions);

  for (const v of values || []) addRow(v);

  return {
    wrap,
    setValues(newValues) {
      list.innerHTML = '';
      for (const v of newValues || []) addRow(v);
    },
    getValues() {
      const out = [];
      for (const row of Array.from(list.children)) {
        const inp = row.querySelector('input');
        const val = inp ? inp.value.trim() : '';
        if (val) out.push(val);
      }
      return out;
    },
    setEnabled(enabled) {
      addBtn.disabled = !enabled;
      for (const row of Array.from(list.children)) {
        const inp = row.querySelector('input');
        const rm = row.querySelector('button');
        if (inp) inp.disabled = !enabled;
        if (rm) rm.disabled = !enabled;
      }
    },
  };
}

function buildConfigCards() {
  const container = byId('configCards');
  if (!container) return;

  container.innerHTML = '';

  const defs = [
    {
      key: 'cpu',
      title: 'CPU config',
      endpoint: '/config/cpu',
      fields: [
        {
          name: 'mode',
          type: 'select',
          label: 'Curve mode',
          options: [
            { value: 'linear', label: 'linear' },
            { value: 'burst', label: 'burst' },
            { value: 's-curve', label: 's-curve' },
          ],
        },
        { name: 'max_value', type: 'number', label: 'max_value (millicores)', min: 1 },
        { name: 'start_value', type: 'number', label: 'start_value (millicores)', min: 0 },
        { name: 'growth_rate', type: 'number', label: 'growth_rate (millicores/sec)', min: 1 },
        { name: 'midpoint_ms', type: 'number', label: 'midpoint_ms', min: 1000 },
        { name: 'interval', type: 'number', label: 'interval (sec)', min: 0 },
      ],
    },
    {
      key: 'memory',
      title: 'Memory config',
      endpoint: '/config/memory',
      fields: [
        {
          name: 'mode',
          type: 'select',
          label: 'Curve mode',
          options: [
            { value: 'linear', label: 'linear' },
            { value: 'burst', label: 'burst' },
            { value: 's-curve', label: 's-curve' },
          ],
        },
        { name: 'target_mb', type: 'number', label: 'target_mb', min: 1 },
        { name: 'start_mb', type: 'number', label: 'start_mb', min: 0 },
        { name: 'growth_rate', type: 'number', label: 'growth_rate (MB/sec)', min: 1 },
        { name: 'midpoint_ms', type: 'number', label: 'midpoint_ms', min: 1000 },
        { name: 'interval', type: 'number', label: 'interval (sec)', min: 0 },
      ],
    },
    {
      key: 'network',
      title: 'Network config',
      endpoint: '/config/network',
      fields: [
        { name: 'endpoint', type: 'text', label: 'endpoint', placeholder: 'http://…' },
        {
          name: 'protocol',
          type: 'select',
          label: 'protocol',
          options: [
            { value: 'http', label: 'http' },
            { value: 'tcp', label: 'tcp' },
            { value: 'udp', label: 'udp' },
          ],
        },
        { name: 'connections', type: 'number', label: 'connections', min: 1 },
        { name: 'midpoint_ms', type: 'number', label: 'midpoint_ms', min: 1000 },
        { name: 'interval', type: 'number', label: 'interval (sec)', min: 0 },
      ],
    },
    {
      key: 'disk',
      title: 'Disk config',
      endpoint: '/config/disk',
      fields: [
        {
          name: 'mode',
          type: 'select',
          label: 'Curve mode',
          options: [
            { value: 'linear', label: 'linear' },
            { value: 'burst', label: 'burst' },
            { value: 's-curve', label: 's-curve' },
          ],
        },
        { name: 'target_mbps', type: 'number', label: 'target_mbps', min: 1 },
        { name: 'start_mbps', type: 'number', label: 'start_mbps', min: 1 },
        {
          name: 'pattern',
          type: 'select',
          label: 'pattern',
          options: [
            { value: 'sequential', label: 'sequential' },
            { value: 'random', label: 'random' },
          ],
        },
        { name: 'read_ratio', type: 'number', label: 'read_ratio (0..1)', min: 0, max: 1, step: 0.1 },
        { name: 'block_size_kb', type: 'number', label: 'block_size_kb', min: 1, max: 1024 },
        { name: 'work_dir', type: 'text', label: 'work_dir', placeholder: '/tmp/k8s-stressor' },
        { name: 'max_file_size_mb', type: 'number', label: 'max_file_size_mb', min: 1 },
        { name: 'midpoint_ms', type: 'number', label: 'midpoint_ms', min: 1000 },
        { name: 'interval', type: 'number', label: 'interval (sec)', min: 0 },
      ],
      listFields: [
        {
          name: 'additional_paths',
          label: 'additional_paths',
          placeholder: '/mnt/pvc-a',
          hint: 'Optional. Add one path per volume for multi-volume tests.',
        },
      ],
    },
    {
      key: 'chaos',
      title: 'Chaos config',
      endpoint: '/config/chaos',
      fields: [
        {
          name: 'fail_liveness',
          type: 'checkbox',
          label: 'fail_liveness',
          hint: 'When enabled, /health returns 503 (simulates liveness probe failure).',
        },
        {
          name: 'fail_readiness',
          type: 'checkbox',
          label: 'fail_readiness',
          hint: 'When enabled, /ready returns 503 (simulates readiness probe failure).',
        },
        {
          name: 'termination_delay_seconds',
          type: 'number',
          label: 'termination_delay_seconds',
          min: 0,
        },
      ],
    },
  ];

  ui.forms = {};

  for (const def of defs) {
    const card = document.createElement('div');
    card.className = 'card';

    const header = document.createElement('div');
    header.className = 'card__header';

    const title = document.createElement('h3');
    title.className = 'card__title';
    title.textContent = def.title;

    const actions = document.createElement('div');
    actions.className = 'inline';

    const editBtn = document.createElement('button');
    editBtn.type = 'button';
    editBtn.className = 'smallBtn';
    editBtn.textContent = 'Edit';

    const applyBtn = document.createElement('button');
    applyBtn.type = 'button';
    applyBtn.className = 'smallBtn';
    applyBtn.textContent = 'Apply';
    applyBtn.disabled = true;

    const cancelBtn = document.createElement('button');
    cancelBtn.type = 'button';
    cancelBtn.className = 'smallBtn';
    cancelBtn.textContent = 'Cancel';
    cancelBtn.disabled = true;

    actions.appendChild(editBtn);
    actions.appendChild(applyBtn);
    actions.appendChild(cancelBtn);

    header.appendChild(title);
    header.appendChild(actions);
    card.appendChild(header);

    const form = document.createElement('div');
    form.className = 'form';

    const grid = document.createElement('div');
    grid.className = 'formGrid';

    const fieldRefs = {};

    for (const f of def.fields) {
      const { wrap, input } = createField(f, null);
      grid.appendChild(wrap);
      fieldRefs[f.name] = { def: f, input };
    }

    form.appendChild(grid);

    const listRefs = {};
    if (def.listFields) {
      for (const lf of def.listFields) {
        const list = createStringList(lf, []);
        form.appendChild(list.wrap);
        listRefs[lf.name] = list;
      }
    }

    const msg = document.createElement('div');
    msg.className = 'msg';
    form.appendChild(msg);

    card.appendChild(form);
    container.appendChild(card);

    function setEnabled(enabled) {
      for (const k of Object.keys(fieldRefs)) {
        const r = fieldRefs[k];
        if (r.def.type === 'checkbox') r.input.disabled = !enabled;
        else r.input.disabled = !enabled;
      }
      for (const k of Object.keys(listRefs)) listRefs[k].setEnabled(enabled);
      applyBtn.disabled = !enabled;
      cancelBtn.disabled = !enabled;
    }

    function setMsg(kind, text) {
      msg.className = 'msg';
      if (kind === 'ok') msg.classList.add('msg--ok');
      if (kind === 'err') msg.classList.add('msg--err');
      msg.textContent = text || '';
    }

    function setValues(values) {
      for (const k of Object.keys(fieldRefs)) {
        const r = fieldRefs[k];
        const v = values ? values[k] : null;
        if (r.def.type === 'checkbox') r.input.checked = Boolean(v);
        else r.input.value = v != null ? String(v) : '';
      }
      for (const k of Object.keys(listRefs)) {
        listRefs[k].setValues(values ? values[k] : []);
      }
    }

    function getPayload() {
      const payload = {};
      for (const k of Object.keys(fieldRefs)) {
        const r = fieldRefs[k];
        if (r.def.type === 'checkbox') {
          payload[k] = Boolean(r.input.checked);
        } else if (r.def.type === 'number') {
          payload[k] = r.input.value === '' ? 0 : Number(r.input.value);
        } else {
          payload[k] = r.input.value;
        }
      }
      for (const k of Object.keys(listRefs)) {
        payload[k] = listRefs[k].getValues();
      }

      // TODO(ui-validation): add client-side validation layer (min/max, required fields)
      // For now we rely on server-side validation errors and show them inline.
      return payload;
    }

    editBtn.addEventListener('click', () => {
      ui.editing[def.key] = true;
      setEnabled(true);
      setMsg('', '');
      if (ui.summary) setValues(ui.summary.configs[def.key]);
    });

    cancelBtn.addEventListener('click', () => {
      ui.editing[def.key] = false;
      setEnabled(false);
      setMsg('', '');
      if (ui.summary) setValues(ui.summary.configs[def.key]);
    });

    applyBtn.addEventListener('click', async () => {
      try {
        setMsg('', 'Applying…');
        const payload = getPayload();
        await jsonFetch(def.endpoint, {
          method: 'PUT',
          body: JSON.stringify(payload),
        });
        setMsg('ok', 'Updated');
        ui.editing[def.key] = false;
        setEnabled(false);
      } catch (e) {
        setMsg('err', e && e.message ? e.message : 'Failed');
      }
    });

    setEnabled(false);

    ui.forms[def.key] = {
      setValues,
      setEnabled,
      setMsg,
      isEditing: () => ui.editing[def.key],
    };
  }
}

function renderSummary(summary) {
  ui.summary = summary;

  setText(byId('appVersion'), `v${summary.version}`);

  const mode = summary.status.mode;
  setText(byId('statusMode'), mode);
  setText(byId('statusConfigVersion'), fmtNumber(summary.status.config_version));
  setText(byId('statusActive'), summary.status.is_active ? 'yes' : 'no');

  const modeSelect = byId('modeSelect');
  if (modeSelect && !modeSelect.matches(':focus')) {
    modeSelect.value = mode;
  }

  setText(byId('lastUpdate'), fmtTime(summary.timestamp_ms));

  // Metrics
  const map = {
    m_cpu_target: `${fmtNumber(summary.metrics.cpu_target_millicores)} m`,
    m_cpu_threads: fmtNumber(summary.metrics.cpu_active_threads),
    m_cpu_cycles: fmtNumber(summary.metrics.cpu_cycle_count),
    m_mem_target: fmtBytes(summary.metrics.memory_target_bytes),
    m_mem_alloc: fmtBytes(summary.metrics.memory_allocated_bytes),
    m_mem_cycles: fmtNumber(summary.metrics.memory_cycle_count),
    m_net_target: fmtNumber(summary.configs.network.connections),
    m_net_active: fmtNumber(summary.metrics.network_active_connections),
    m_net_req: fmtNumber(summary.metrics.network_requests_total),
    m_net_err: fmtNumber(summary.metrics.network_errors_total),
    m_net_cycles: fmtNumber(summary.metrics.network_cycle_count),
    m_disk_target: `${fmtNumber(summary.metrics.disk_target_mbps)} MB/s`,
    m_disk_actual: `${fmtNumber(summary.metrics.disk_actual_mbps)} MB/s`,
    m_disk_w: fmtBytes(summary.metrics.disk_bytes_written_total),
    m_disk_r: fmtBytes(summary.metrics.disk_bytes_read_total),
    m_disk_err: fmtNumber(summary.metrics.disk_io_errors_total),
    m_disk_cycles: fmtNumber(summary.metrics.disk_cycle_count),
  };
  for (const id of Object.keys(map)) setText(byId(id), map[id]);

  // Configs
  for (const key of Object.keys(ui.forms)) {
    const form = ui.forms[key];
    if (!form) continue;
    if (form.isEditing()) continue;
    form.setValues(summary.configs[key]);
  }
}

function initControls() {
  const msg = byId('controlMsg');

  function setMsg(kind, text) {
    if (!msg) return;
    msg.className = 'msg';
    if (kind === 'ok') msg.classList.add('msg--ok');
    if (kind === 'err') msg.classList.add('msg--err');
    msg.textContent = text || '';
  }

  const applyBtn = byId('applyModeBtn');
  const stopBtn = byId('stopBtn');
  const modeSelect = byId('modeSelect');

  if (applyBtn) {
    applyBtn.addEventListener('click', async () => {
      try {
        setMsg('', 'Applying mode…');
        const mode = modeSelect ? modeSelect.value : 'idle';
        await jsonFetch('/mode', {
          method: 'PUT',
          body: JSON.stringify(mode),
        });
        setMsg('ok', 'Mode updated');
      } catch (e) {
        setMsg('err', e && e.message ? e.message : 'Failed');
      }
    });
  }

  if (stopBtn) {
    stopBtn.addEventListener('click', async () => {
      try {
        setMsg('', 'Stopping…');
        await jsonFetch('/stop', { method: 'PUT' });
        setMsg('ok', 'Stopped');
      } catch (e) {
        setMsg('err', e && e.message ? e.message : 'Failed');
      }
    });
  }
}

function connectSse() {
  const statusPill = byId('connStatus');

  const es = new EventSource('/api/events');

  es.onopen = () => {
    setPill(statusPill, 'Connected', 'good');
  };

  es.onerror = () => {
    // EventSource will auto-reconnect.
    setPill(statusPill, 'Reconnecting…', 'bad');
  };

  es.addEventListener('summary', (ev) => {
    try {
      const summary = JSON.parse(ev.data);
      renderSummary(summary);
    } catch {
      // ignore
    }
  });

  return es;
}

async function bootstrap() {
  buildMetricCards();
  buildConfigCards();
  initControls();

  try {
    const summary = await jsonFetch('/api/ui/summary');
    renderSummary(summary);
  } catch {
    // ignore; SSE will eventually update when available
  }

  connectSse();
}

bootstrap();
