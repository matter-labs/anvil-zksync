import React from 'react';

export const Flag: React.FC<{ name: string; type: string; children?: React.ReactNode }> = ({
  name,
  type,
  children,
}) => (
  <tr className="align-top">
    <td>
      <code>{name}</code>
    </td>
    <td className="pr-3">{type}</td>
    <td>{children}</td>
  </tr>
);
