import type { ReactNode } from "react";

export type FormValue = string | string[];
export type FormValues = Record<string, FormValue>;

export type FormField =
  | { kind: "group"; id: string; label: string; description?: string; children: FormField[] }
  | { kind: "text" | "date" | "number"; id: string; label: string; placeholder?: string; step?: string }
  | { kind: "select"; id: string; label: string; options: Array<{ value: string; label: string }> }
  | { kind: "multi"; id: string; label: string; options: Array<{ value: string; label: string }> };

interface SchemaFormProps {
  fields: FormField[];
  values: FormValues;
  onChange: (id: string, value: FormValue) => void;
}

function renderField(field: FormField, values: FormValues, onChange: SchemaFormProps["onChange"]): ReactNode {
  if (field.kind === "group") {
    return (
      <fieldset className="schema-group" key={field.id}>
        <legend>{field.label}</legend>
        {field.description && <p>{field.description}</p>}
        <div className="schema-grid">{field.children.map((child) => renderField(child, values, onChange))}</div>
      </fieldset>
    );
  }
  if (field.kind === "multi") {
    const selected = Array.isArray(values[field.id]) ? values[field.id] as string[] : [];
    return (
      <fieldset className="schema-multi" key={field.id}>
        <legend>{field.label}</legend>
        {field.options.map((option) => (
          <label key={option.value}>
            <input
              type="checkbox"
              checked={selected.includes(option.value)}
              onChange={(event) => onChange(
                field.id,
                event.target.checked
                  ? [...selected, option.value]
                  : selected.filter((value) => value !== option.value),
              )}
            />
            <span>{option.label}</span>
          </label>
        ))}
      </fieldset>
    );
  }
  if (field.kind === "select") {
    return (
      <label className="field-label" key={field.id}>
        {field.label}
        <select value={String(values[field.id] ?? "")} onChange={(event) => onChange(field.id, event.target.value)}>
          {field.options.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
        </select>
      </label>
    );
  }
  return (
    <label className="field-label" key={field.id}>
      {field.label}
      <input
        type={field.kind}
        step={field.step}
        placeholder={field.placeholder}
        value={String(values[field.id] ?? "")}
        onChange={(event) => onChange(field.id, event.target.value)}
      />
    </label>
  );
}

export function SchemaForm({ fields, values, onChange }: SchemaFormProps) {
  return <div className="schema-form">{fields.map((field) => renderField(field, values, onChange))}</div>;
}
