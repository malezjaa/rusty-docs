import Link from 'next/link';
import { appName, appDescription } from '@/lib/shared';

export default function HomePage() {
  return (
    <div className="flex flex-col justify-center text-center flex-1">
      <h1 className="text-2xl font-bold mb-4">{appName}</h1>
      <p>{appDescription}</p>
      <p>
        Browse the{' '}
        <Link href="/docs" className="font-medium underline">
          /docs
        </Link>{' '}
        and see the documentation.
      </p>
    </div>
  );
}
