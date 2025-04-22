export function OptionTable({ children }: { children: React.ReactNode }) {
  return (
    <table className="w-full text-sm border-collapse my-6">
      <thead>
        <tr className="border-b">
          <th className="text-left py-2 font-semibold">Flag</th>
          <th className="text-left py-2 font-semibold">Type</th>
          <th className="text-left py-2 font-semibold">Description</th>
        </tr>
      </thead>
      <tbody>{children}</tbody>
    </table>
  );
}