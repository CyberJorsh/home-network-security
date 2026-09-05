import type { ReactNode } from 'react';
function inline(text: string): ReactNode[] {
  return text
    .split(/(\*\*[^*]+\*\*|`[^`]+`)/g)
    .map((part, i) =>
      part.startsWith('**') ? (
        <strong key={i}>{part.slice(2, -2)}</strong>
      ) : part.startsWith('`') ? (
        <code key={i}>{part.slice(1, -1)}</code>
      ) : (
        part
      ),
    );
}
// A deliberately small Markdown subset: no HTML, remote images, or executable links.
export default function SafeResponse({ text }: { text: string }) {
  const blocks = text.split(/(```[^\n]*\n[\s\S]*?(?:```|$))/g);
  return (
    <div className="formatted-response">
      {blocks.map((block, b) =>
        block.startsWith('```') ? (
          <pre key={b}>
            <code>{block.replace(/^```[^\n]*\n/, '').replace(/```$/, '')}</code>
          </pre>
        ) : (
          block
            .split(/\n\s*\n/)
            .filter(Boolean)
            .map((paragraph, p) => {
              const lines = paragraph.split('\n');
              if (lines.every((line) => /^\s*[-*] /.test(line)))
                return (
                  <ul key={`${b}-${p}`}>
                    {lines.map((line, l) => (
                      <li key={l}>{inline(line.replace(/^\s*[-*] /, ''))}</li>
                    ))}
                  </ul>
                );
              if (/^#{1,6} /.test(paragraph))
                return (
                  <h3 key={`${b}-${p}`}>
                    {inline(paragraph.replace(/^#{1,6} /, ''))}
                  </h3>
                );
              return <p key={`${b}-${p}`}>{inline(paragraph)}</p>;
            })
        ),
      )}
    </div>
  );
}
