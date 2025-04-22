import React from "react";
import { Command } from "./Command";

interface RowProps {
  flag: string;
  type: string;
  defaultValue?: string;
  children: React.ReactNode;
}

export function Row({ flag, type, defaultValue, children }: RowProps) {
  return (
    <tr className="align-top">
      <td className="py-2">
        <Command>{flag}</Command>
        {defaultValue && (
          <span className="text-xs opacity-60 ml-1">
            (default&nbsp;{defaultValue})
          </span>
        )}
      </td>
      <td className="py-2 whitespace-nowrap">{type}</td>
      <td className="py-2">{children}</td>
    </tr>
  );
}