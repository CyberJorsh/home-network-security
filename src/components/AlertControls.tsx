import { useEffect, useState } from 'react';
import { command, native } from '../api';

type Rules = { uploadEnabled: boolean; uploadThresholdMib: number };
export default function AlertControls({
  onChanged,
  onError,
}: {
  onChanged: () => void;
  onError: (message: string) => void;
}) {
  const [rules, setRules] = useState<Rules>();
  const [threshold, setThreshold] = useState('50');
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('');
  useEffect(() => {
    let alive = true;
    if (native)
      void command<Rules>('alert_rules')
        .then((value) => {
          if (alive) {
            setRules(value);
            setThreshold(String(value.uploadThresholdMib));
          }
        })
        .catch((e) => {
          if (alive) onError(String(e));
        });
    return () => {
      alive = false;
    };
  }, [onError]);
  const value = Number(threshold);
  const valid = Number.isInteger(value) && value >= 1 && value <= 1048576;
  const save = async () => {
    if (!rules || !valid) return;
    setBusy(true);
    setNotice('');
    try {
      await command('set_alert_rules', {
        rules: { ...rules, uploadThresholdMib: value },
      });
      setNotice('Alert rules saved for this computer.');
      onChanged();
    } catch (e) {
      onError(String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <details className="panel setup-panel">
      <summary>Local alert rules</summary>
      <p>
        These rules apply to this computer’s observations. Remote collectors
        keep their own rules. Upload notices describe observed volume, not
        malicious activity.
      </p>
      <label>
        <input
          type="checkbox"
          checked={rules?.uploadEnabled ?? true}
          disabled={!rules || busy}
          onChange={(e) =>
            setRules((r) => r && { ...r, uploadEnabled: e.target.checked })
          }
        />{' '}
        Enable large upload notices
      </label>
      <label htmlFor="upload-threshold">
        Upload threshold (MiB per UTC hour)
      </label>
      <input
        id="upload-threshold"
        type="number"
        min="1"
        max="1048576"
        step="1"
        value={threshold}
        disabled={!rules || busy}
        onChange={(e) => setThreshold(e.target.value)}
      />
      <p>
        Between 1 and 1,048,576 MiB. Backups, updates and video calls may
        legitimately exceed this limit. A filtered view can contain only part of
        an hour; inspect the supporting conversations before acting.
      </p>
      <button
        className="button secondary"
        disabled={!rules || busy || !valid}
        onClick={() => void save()}
      >
        Save alert rules
      </button>
      {notice && <p role="status">{notice}</p>}
    </details>
  );
}
