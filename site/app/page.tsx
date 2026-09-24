import { SiteHeader } from "@/components/site-header";
import { Hero } from "@/components/hero";
import { LogoStrip } from "@/components/logo-strip";
import { WhySection } from "@/components/why-section";
import { FeaturesSection } from "@/components/features-section";
import { HowItWorks } from "@/components/how-it-works";
import { InterfacesSection } from "@/components/interfaces-section";
import { InstallSection } from "@/components/install-section";
import { CliReference } from "@/components/cli-reference";
import { DemoSection } from "@/components/demo-section";
import { CtaSection } from "@/components/cta-section";
import { SiteFooter } from "@/components/site-footer";

export default function Home() {
  return (
    <>
      <div className="bg-noise pointer-events-none fixed inset-0 z-0" aria-hidden="true" />
      <div id="top">
        <SiteHeader />
        <main>
          <Hero />
          <LogoStrip />
          <WhySection />
          <FeaturesSection />
          <HowItWorks />
          <InterfacesSection />
          <InstallSection />
          <CliReference />
          <DemoSection />
          <CtaSection />
        </main>
        <SiteFooter />
      </div>
    </>
  );
}
